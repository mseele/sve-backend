use crate::models::{Event, EventType, NewsType, PartialEvent, Subscription};
use anyhow::{anyhow, bail, Context, Result};
use chrono::{NaiveDateTime, Utc};
use googapis::{
    google::firestore::v1::{
        document_transform::{field_transform::TransformType, FieldTransform},
        firestore_client::FirestoreClient,
        get_document_request::ConsistencySelector,
        value::ValueType,
        write::Operation,
        ArrayValue, BeginTransactionRequest, CommitRequest, CreateDocumentRequest,
        DeleteDocumentRequest, Document, DocumentMask, DocumentTransform, GetDocumentRequest,
        ListDocumentsRequest, RollbackRequest, UpdateDocumentRequest, Value, Write,
    },
    CERTIFICATES,
};
use gouth::Builder;
use std::{collections::HashMap, str::FromStr};
use tonic::{
    codegen::InterceptedService,
    metadata::MetadataValue,
    service::Interceptor,
    transport::{Certificate, Channel, ClientTlsConfig},
    Code, Request, Status,
};

const PROJECT: &str = "sve-backend";

pub struct GouthInterceptor;

impl Interceptor for GouthInterceptor {
    fn call(&mut self, mut request: tonic::Request<()>) -> Result<tonic::Request<()>, Status> {
        let token = Builder::new()
            .json(crate::CREDENTIALS)
            .build()
            .map_err(|e| Status::new(Code::Internal, format!("Could not create token: {}", e)))?;
        let header_value = token.header_value().map_err(|e| {
            Status::new(
                Code::Internal,
                format!("Could not extract header value: {}", e),
            )
        })?;
        let meta = MetadataValue::from_str(&header_value)
            .map_err(|e| Status::new(Code::Internal, format!("Could not create meta: {}", e)))?;
        request.metadata_mut().insert("authorization", meta);
        Ok(request)
    }
}

pub async fn get_client() -> Result<FirestoreClient<InterceptedService<Channel, GouthInterceptor>>>
{
    let tls_config = ClientTlsConfig::new()
        .ca_certificate(Certificate::from_pem(CERTIFICATES))
        .domain_name("firestore.googleapis.com");

    let channel = Channel::from_static("https://firestore.googleapis.com")
        .tls_config(tls_config)?
        .connect()
        .await?;

    Ok(FirestoreClient::with_interceptor(channel, GouthInterceptor))
}

pub async fn get_events(
    client: &mut FirestoreClient<InterceptedService<Channel, GouthInterceptor>>,
) -> Result<Vec<Event>> {
    let response = client
        .list_documents(Request::new(ListDocumentsRequest {
            parent: format!("projects/{}/databases/(default)/documents", PROJECT),
            collection_id: "events".into(),
            page_size: 100,
            ..Default::default()
        }))
        .await?;

    let events = response
        .into_inner()
        .documents
        .into_iter()
        .map(|mut doc| to_event(&mut doc))
        .collect::<Result<Vec<_>>>()?;

    Ok(events)
}

pub async fn get_event(
    client: &mut FirestoreClient<InterceptedService<Channel, GouthInterceptor>>,
    id: &str,
) -> Result<Event> {
    let response = client
        .get_document(Request::new(GetDocumentRequest {
            name: format!(
                "projects/{}/databases/(default)/documents/events/{}",
                PROJECT, id
            ),
            ..Default::default()
        }))
        .await?;

    let event = to_event(&mut response.into_inner())?;

    Ok(event)
}

pub async fn write_event(
    client: &mut FirestoreClient<InterceptedService<Channel, GouthInterceptor>>,
    partial_event: PartialEvent,
) -> Result<Event> {
    let get_document_result = client
        .get_document(Request::new(GetDocumentRequest {
            name: format!(
                "projects/{}/databases/(default)/documents/events/{}",
                PROJECT, partial_event.id
            ),
            ..Default::default()
        }))
        .await;
    let document_result: Result<Option<Document>> = match get_document_result {
        Ok(response) => Ok(Some(response.into_inner())),
        Err(status) if status.code() == Code::NotFound => Ok(None),
        Err(status) => Err(status.into()),
    };
    let event;
    let document_result = document_result?;
    if document_result.is_some() {
        event = update_event(client, partial_event).await?;
    } else {
        event = create_event(client, partial_event.try_into()?).await?;
    }

    Ok(event)
}

pub async fn delete_event(
    client: &mut FirestoreClient<InterceptedService<Channel, GouthInterceptor>>,
    id: &str,
) -> Result<()> {
    client
        .delete_document(Request::new(DeleteDocumentRequest {
            name: format!(
                "projects/{}/databases/(default)/documents/events/{}",
                PROJECT, id
            ),
            ..Default::default()
        }))
        .await?;

    Ok(())
}

pub enum BookingResult {
    Booked(Event, String),
    WaitingList(Event, String),
    BookedOut,
}

pub async fn book_event(
    client: &mut FirestoreClient<InterceptedService<Channel, GouthInterceptor>>,
    id: &str,
) -> Result<BookingResult> {
    // begin transaction
    let response = client
        .begin_transaction(Request::new(BeginTransactionRequest {
            database: format!("projects/{}/databases/(default)", PROJECT),
            ..Default::default()
        }))
        .await?;
    let transaction = response.into_inner().transaction;

    match run_book_event_transaction(client, id, transaction.clone()).await {
        Ok(result) => {
            match result {
                BookingTransaction::Book(writes, booking_number) => {
                    // commit transaction
                    commit_transaction(client, writes, transaction).await?;
                    let event = get_event(client, id).await?;
                    Ok(BookingResult::Booked(event, booking_number))
                }
                BookingTransaction::WaitingList(writes, booking_number) => {
                    // commit transaction
                    commit_transaction(client, writes, transaction).await?;
                    let event = get_event(client, id).await?;
                    Ok(BookingResult::WaitingList(event, booking_number))
                }
                BookingTransaction::BookedOut => {
                    // rollback transaction
                    rollback_transaction(client, transaction).await?;
                    Ok(BookingResult::BookedOut)
                }
            }
        }
        Err(err) => {
            // rollback transaction
            rollback_transaction(client, transaction).await?;
            Err(err)
        }
    }
}

pub enum BookingTransaction {
    Book(Vec<Write>, String),
    WaitingList(Vec<Write>, String),
    BookedOut,
}

async fn run_book_event_transaction(
    client: &mut FirestoreClient<InterceptedService<Channel, GouthInterceptor>>,
    id: &str,
    transaction: Vec<u8>,
) -> Result<BookingTransaction> {
    // get booking fields
    let subscribers_field = "subscribers";
    let max_subscribers_field = "maxSubscribers";
    let waiting_list_field = "waitingList";
    let max_waiting_list_field = "maxWaitingList";

    let response = client
        .get_document(Request::new(GetDocumentRequest {
            name: format!(
                "projects/{}/databases/(default)/documents/events/{}",
                PROJECT, id
            ),
            mask: Some(DocumentMask {
                field_paths: vec![
                    subscribers_field.into(),
                    max_subscribers_field.into(),
                    waiting_list_field.into(),
                    max_waiting_list_field.into(),
                ],
            }),
            consistency_selector: Some(ConsistencySelector::Transaction(transaction.clone())),
            ..Default::default()
        }))
        .await?;

    // extract and increment counters
    let mut doc = response.into_inner();
    let subscribers = get_integer(&mut doc, subscribers_field)?;
    let max_subscribers = get_integer(&mut doc, max_subscribers_field)?;
    if max_subscribers == -1 || subscribers < max_subscribers {
        let mut writes = Vec::new();
        // generate booking number
        let (write, booking_number) = get_next_booking_number(client, transaction).await?;
        writes.push(write);

        writes.push(Write {
            operation: Some(Operation::Transform(DocumentTransform {
                document: format!(
                    "projects/{}/databases/(default)/documents/events/{}",
                    PROJECT, id
                ),
                field_transforms: vec![FieldTransform {
                    field_path: subscribers_field.into(),
                    transform_type: Some(TransformType::Increment(Value {
                        value_type: Some(ValueType::IntegerValue(1)),
                    })),
                }],
            })),
            update_mask: None,
            update_transforms: vec![],
            current_document: None,
        });

        return Ok(BookingTransaction::Book(writes, booking_number));
    }

    let waiting_list = get_integer(&mut doc, waiting_list_field)?;
    let max_waiting_list = get_integer(&mut doc, max_waiting_list_field)?;
    if waiting_list < max_waiting_list {
        let mut writes = Vec::new();

        // generate booking number
        let (write, booking_number) = get_next_booking_number(client, transaction.clone()).await?;
        writes.push(write);

        writes.push(Write {
            operation: Some(Operation::Transform(DocumentTransform {
                document: format!(
                    "projects/{}/databases/(default)/documents/events/{}",
                    PROJECT, id
                ),
                field_transforms: vec![FieldTransform {
                    field_path: waiting_list_field.into(),
                    transform_type: Some(TransformType::Increment(Value {
                        value_type: Some(ValueType::IntegerValue(1)),
                    })),
                }],
            })),
            update_mask: None,
            update_transforms: vec![],
            current_document: None,
        });

        return Ok(BookingTransaction::WaitingList(writes, booking_number));
    }

    Ok(BookingTransaction::BookedOut)
}

async fn get_next_booking_number(
    client: &mut FirestoreClient<InterceptedService<Channel, GouthInterceptor>>,
    transaction: Vec<u8>,
) -> Result<(Write, String)> {
    let id_field = "id";
    let response = client
        .get_document(Request::new(GetDocumentRequest {
            name: format!(
                "projects/{}/databases/(default)/documents/counters/events",
                PROJECT
            ),
            mask: Some(DocumentMask {
                field_paths: vec![id_field.into()],
            }),
            consistency_selector: Some(ConsistencySelector::Transaction(transaction)),
            ..Default::default()
        }))
        .await?;

    let id = get_integer(&mut response.into_inner(), id_field)?;

    // next_id need to be between 1000 & 10000
    let mut next_id = id;
    let transform_type;
    if id < 1000 || id >= 9999 {
        transform_type = TransformType::Minimum(Value {
            value_type: Some(ValueType::IntegerValue(1000)),
        });
        next_id = 1000;
    } else {
        transform_type = TransformType::Increment(Value {
            value_type: Some(ValueType::IntegerValue(1)),
        });
        next_id += 1;
    }

    let write = Write {
        operation: Some(Operation::Transform(DocumentTransform {
            document: format!(
                "projects/{}/databases/(default)/documents/counters/events",
                PROJECT
            ),
            field_transforms: vec![FieldTransform {
                field_path: id_field.into(),
                transform_type: Some(transform_type),
            }],
        })),
        update_mask: None,
        update_transforms: vec![],
        current_document: None,
    };

    let next_booking_number = format!("{}-{:04}", Utc::now().format("%y"), next_id);

    Ok((write, next_booking_number))
}

pub async fn get_subscriptions(
    client: &mut FirestoreClient<InterceptedService<Channel, GouthInterceptor>>,
) -> Result<Vec<Subscription>> {
    let mut subscriptions: Vec<Subscription> = Vec::new();

    let mut next_page_token = String::from("");
    loop {
        let response = client
            .list_documents(Request::new(ListDocumentsRequest {
                parent: format!("projects/{}/databases/(default)/documents", PROJECT),
                collection_id: "subscriptions".into(),
                page_size: 500,
                page_token: next_page_token,
                ..Default::default()
            }))
            .await?
            .into_inner();

        for mut document in response.documents {
            subscriptions.push(to_subscription(&mut document)?);
        }

        next_page_token = response.next_page_token;
        if next_page_token.is_empty() {
            break;
        }
    }

    Ok(subscriptions)
}

pub async fn subscribe(
    client: &mut FirestoreClient<InterceptedService<Channel, GouthInterceptor>>,
    subscription: &Subscription,
) -> Result<Subscription> {
    let email = subscription.email.as_str();
    match get_subscription(client, email).await? {
        Some(existing_subscription) => {
            let mut types = subscription.types.clone();
            // add existing types
            for existing_type in existing_subscription.types {
                if !types.contains(&existing_type) {
                    types.push(existing_type);
                }
            }
            // remove duplicates
            types.dedup();

            update_subscription(client, &subscription.email, &types).await?;

            Ok(Subscription::new(subscription.email.clone(), types))
        }
        None => {
            let mut fields: HashMap<String, Value> = HashMap::new();
            insert_news_type_values(&mut fields, "types", subscription.types.clone());
            client
                .create_document(Request::new(CreateDocumentRequest {
                    parent: format!("projects/{}/databases/(default)/documents", PROJECT),
                    collection_id: "subscriptions".into(),
                    document_id: subscription.email.clone(),
                    document: Some(Document {
                        fields,
                        ..Default::default()
                    }),
                    ..Default::default()
                }))
                .await?;

            Ok(subscription.clone())
        }
    }
}

pub async fn unsubscribe(
    client: &mut FirestoreClient<InterceptedService<Channel, GouthInterceptor>>,
    subscription: &Subscription,
) -> Result<()> {
    let email = subscription.email.as_str();
    match get_subscription(client, email).await? {
        Some(existing_subscription) => {
            let types: Vec<NewsType> = existing_subscription
                .types
                .clone()
                .into_iter()
                .filter(|t| !subscription.types.contains(t))
                .collect();
            if types.len() > 0 {
                update_subscription(client, &subscription.email, &types).await?;
            } else {
                client
                    .delete_document(Request::new(DeleteDocumentRequest {
                        name: format!(
                            "projects/{}/databases/(default)/documents/subscriptions/{}",
                            PROJECT, email
                        ),
                        ..Default::default()
                    }))
                    .await?;
            }
        }
        // email has not been registered before - nothing to do
        None => (),
    }

    Ok(())
}

async fn update_subscription(
    client: &mut FirestoreClient<InterceptedService<Channel, GouthInterceptor>>,
    email: &str,
    types: &Vec<NewsType>,
) -> Result<()> {
    let mut fields: HashMap<String, Value> = HashMap::new();
    insert_news_type_values(&mut fields, "types", types.clone());

    let update_mask = DocumentMask {
        field_paths: fields.keys().cloned().collect(),
    };

    let document = Document {
        name: format!(
            "projects/{}/databases/(default)/documents/subscriptions/{}",
            PROJECT, email
        ),
        fields,
        ..Default::default()
    };

    client
        .update_document(Request::new(UpdateDocumentRequest {
            document: Some(document),
            update_mask: Some(update_mask),
            ..Default::default()
        }))
        .await?;

    Ok(())
}

async fn get_subscription(
    client: &mut FirestoreClient<InterceptedService<Channel, GouthInterceptor>>,
    email: &str,
) -> Result<Option<Subscription>> {
    let get_document_result = client
        .get_document(Request::new(GetDocumentRequest {
            name: format!(
                "projects/{}/databases/(default)/documents/subscriptions/{}",
                PROJECT, email
            ),
            ..Default::default()
        }))
        .await;
    match get_document_result {
        Ok(response) => Ok(Some(to_subscription(&mut response.into_inner())?)),
        Err(status) if status.code() == Code::NotFound => Ok(None),
        Err(status) => Err(status.into()),
    }
}

async fn create_event(
    client: &mut FirestoreClient<InterceptedService<Channel, GouthInterceptor>>,
    event: Event,
) -> Result<Event> {
    let response = client
        .create_document(Request::new(CreateDocumentRequest {
            parent: format!("projects/{}/databases/(default)/documents", PROJECT),
            collection_id: "events".into(),
            document_id: event.id.clone(),
            document: Some(from_event(event)),
            ..Default::default()
        }))
        .await?;

    let event = to_event(&mut response.into_inner())?;

    Ok(event)
}

async fn update_event(
    client: &mut FirestoreClient<InterceptedService<Channel, GouthInterceptor>>,
    partial_event: PartialEvent,
) -> Result<Event> {
    let (document, update_mask) = from_partial_event(partial_event);
    let response = client
        .update_document(Request::new(UpdateDocumentRequest {
            document: Some(document),
            update_mask: Some(update_mask),
            ..Default::default()
        }))
        .await?;

    let event = to_event(&mut response.into_inner())?;

    Ok(event)
}

async fn commit_transaction(
    client: &mut FirestoreClient<InterceptedService<Channel, GouthInterceptor>>,
    writes: Vec<Write>,
    transaction: Vec<u8>,
) -> Result<()> {
    client
        .commit(Request::new(CommitRequest {
            database: format!("projects/{}/databases/(default)", PROJECT),
            writes,
            transaction,
        }))
        .await?;

    Ok(())
}

async fn rollback_transaction(
    client: &mut FirestoreClient<InterceptedService<Channel, GouthInterceptor>>,
    transaction: Vec<u8>,
) -> Result<()> {
    client
        .rollback(Request::new(RollbackRequest {
            database: format!("projects/{}/databases/(default)", PROJECT),
            transaction,
        }))
        .await?;

    Ok(())
}

fn from_partial_event(event: PartialEvent) -> (Document, DocumentMask) {
    let mut fields: HashMap<String, Value> = HashMap::new();
    insert_opt_string_value(&mut fields, "sheetId", event.sheet_id);
    insert_opt_integer_value(&mut fields, "gid", event.gid);
    match event.event_type {
        Some(value) => insert_str_value(&mut fields, "type", value.into()),
        None => (),
    }
    insert_opt_string_value(&mut fields, "name", event.name);
    insert_opt_integer_value(&mut fields, "sortIndex", event.sort_index);
    insert_opt_bool_value(&mut fields, "visible", event.visible);
    insert_opt_bool_value(&mut fields, "beta", event.beta);
    insert_opt_string_value(&mut fields, "shortDescription", event.short_description);
    insert_opt_string_value(&mut fields, "description", event.description);
    insert_opt_string_value(&mut fields, "image", event.image);
    insert_opt_bool_value(&mut fields, "light", event.light);
    match event.dates {
        Some(value) => insert_date_values(&mut fields, "dates", value),
        None => (),
    }
    insert_opt_string_value(&mut fields, "customDate", event.custom_date);
    insert_opt_integer_value(&mut fields, "durationInMinutes", event.duration_in_minutes);
    insert_opt_integer_value(&mut fields, "maxSubscribers", event.max_subscribers);
    insert_opt_integer_value(&mut fields, "subscribers", event.subscribers);
    insert_opt_double_value(&mut fields, "costMember", event.cost_member);
    insert_opt_double_value(&mut fields, "costNonMember", event.cost_non_member);
    insert_opt_integer_value(&mut fields, "waitingList", event.waiting_list);
    insert_opt_integer_value(&mut fields, "maxWaitingList", event.max_waiting_list);
    insert_opt_string_value(&mut fields, "location", event.location);
    insert_opt_string_value(&mut fields, "bookingTemplate", event.booking_template);
    insert_opt_string_value(&mut fields, "waitingTemplate", event.waiting_template);
    insert_opt_string_value(
        &mut fields,
        "altBookingButtonText",
        event.alt_booking_button_text,
    );
    insert_opt_string_value(&mut fields, "altEmailAddress", event.alt_email_address);
    insert_opt_bool_value(&mut fields, "externalOperator", event.external_operator);

    let field_paths = fields.keys().cloned().collect();

    (
        Document {
            name: format!(
                "projects/{}/databases/(default)/documents/events/{}",
                PROJECT, event.id
            ),
            fields,
            ..Default::default()
        },
        DocumentMask {
            field_paths: field_paths,
        },
    )
}

fn from_event(event: Event) -> Document {
    let mut fields: HashMap<String, Value> = HashMap::new();
    insert_string_value(&mut fields, "sheetId", event.sheet_id);
    insert_integer_value(&mut fields, "gid", event.gid);
    insert_str_value(&mut fields, "type", event.event_type.into());
    insert_string_value(&mut fields, "name", event.name);
    insert_integer_value(&mut fields, "sortIndex", event.sort_index);
    insert_bool_value(&mut fields, "visible", event.visible);
    insert_bool_value(&mut fields, "beta", event.beta);
    insert_string_value(&mut fields, "shortDescription", event.short_description);
    insert_string_value(&mut fields, "description", event.description);
    insert_string_value(&mut fields, "image", event.image);
    insert_bool_value(&mut fields, "light", event.light);
    insert_date_values(&mut fields, "dates", event.dates);
    insert_opt_string_value(&mut fields, "customDate", event.custom_date);
    insert_integer_value(&mut fields, "durationInMinutes", event.duration_in_minutes);
    insert_integer_value(&mut fields, "maxSubscribers", event.max_subscribers);
    insert_integer_value(&mut fields, "subscribers", event.subscribers);
    insert_double_value(&mut fields, "costMember", event.cost_member);
    insert_double_value(&mut fields, "costNonMember", event.cost_non_member);
    insert_integer_value(&mut fields, "waitingList", event.waiting_list);
    insert_integer_value(&mut fields, "maxWaitingList", event.max_waiting_list);
    insert_string_value(&mut fields, "location", event.location);
    insert_string_value(&mut fields, "bookingTemplate", event.booking_template);
    insert_string_value(&mut fields, "waitingTemplate", event.waiting_template);
    insert_opt_string_value(
        &mut fields,
        "altBookingButtonText",
        event.alt_booking_button_text,
    );
    insert_opt_string_value(&mut fields, "altEmailAddress", event.alt_email_address);
    insert_bool_value(&mut fields, "externalOperator", event.external_operator);

    Document {
        fields,
        ..Default::default()
    }
}

fn insert_opt_string_value(fields: &mut HashMap<String, Value>, key: &str, value: Option<String>) {
    match value {
        Some(v) => insert_string_value(fields, key, v),
        None => (),
    }
}

fn insert_opt_integer_value(fields: &mut HashMap<String, Value>, key: &str, value: Option<i64>) {
    match value {
        Some(v) => insert_integer_value(fields, key, v),
        None => (),
    }
}

fn insert_opt_double_value(fields: &mut HashMap<String, Value>, key: &str, value: Option<f64>) {
    match value {
        Some(v) => insert_double_value(fields, key, v),
        None => (),
    }
}

fn insert_opt_bool_value(fields: &mut HashMap<String, Value>, key: &str, value: Option<bool>) {
    match value {
        Some(v) => insert_bool_value(fields, key, v),
        None => (),
    }
}

fn insert_str_value(fields: &mut HashMap<String, Value>, key: &str, value: &str) {
    insert_string_value(fields, key, value.into());
}

fn insert_string_value(fields: &mut HashMap<String, Value>, key: &str, value: String) {
    fields.insert(
        key.into(),
        Value {
            value_type: Some(ValueType::StringValue(value)),
        },
    );
}

fn insert_integer_value(fields: &mut HashMap<String, Value>, key: &str, value: i64) {
    fields.insert(
        key.into(),
        Value {
            value_type: Some(ValueType::IntegerValue(value)),
        },
    );
}

fn insert_double_value(fields: &mut HashMap<String, Value>, key: &str, value: f64) {
    fields.insert(
        key.into(),
        Value {
            value_type: Some(ValueType::DoubleValue(value)),
        },
    );
}

fn insert_bool_value(fields: &mut HashMap<String, Value>, key: &str, value: bool) {
    fields.insert(
        key.into(),
        Value {
            value_type: Some(ValueType::BooleanValue(value)),
        },
    );
}

fn insert_date_values(fields: &mut HashMap<String, Value>, key: &str, values: Vec<NaiveDateTime>) {
    fields.insert(
        key.into(),
        Value {
            value_type: Some(ValueType::ArrayValue(ArrayValue {
                values: values
                    .iter()
                    .map(|date| Value {
                        value_type: Some(ValueType::StringValue(
                            date.format("%Y-%m-%dT%H:%M").to_string(),
                        )),
                    })
                    .collect(),
            })),
        },
    );
}

fn insert_news_type_values(fields: &mut HashMap<String, Value>, key: &str, values: Vec<NewsType>) {
    fields.insert(
        key.into(),
        Value {
            value_type: Some(ValueType::ArrayValue(ArrayValue {
                values: values
                    .into_iter()
                    .map(|news_type| {
                        let news_string: &str = news_type.into();
                        return Value {
                            value_type: Some(ValueType::StringValue(news_string.to_string())),
                        };
                    })
                    .collect(),
            })),
        },
    );
}

fn to_event(doc: &mut Document) -> Result<Event> {
    let index = doc
        .name
        .rfind('/')
        .with_context(|| format!("Found no / in document name {}", doc.name))?;
    let id = &doc.name[index + 1..];

    let event = Event::new(
        id.into(),
        get_string(doc, "sheetId")?,
        get_integer(doc, "gid")?,
        EventType::from_str(&get_string(doc, "type")?)?,
        get_string(doc, "name")?,
        get_integer(doc, "sortIndex")?,
        get_bool(doc, "visible")?,
        get_bool(doc, "beta")?,
        get_string(doc, "shortDescription")?,
        get_string(doc, "description")?,
        get_string(doc, "image")?,
        get_bool(doc, "light")?,
        get_strings(doc, "dates")?
            .iter()
            .map(|date| {
                NaiveDateTime::parse_from_str(date, "%Y-%m-%dT%H:%M")
                    .with_context(|| format!("Error parsing date string {}", date))
            })
            .collect::<Result<Vec<_>, _>>()?,
        get_opt_string(doc, "customDate")?,
        get_integer(doc, "durationInMinutes")?,
        get_integer(doc, "maxSubscribers")?,
        get_integer(doc, "subscribers")?,
        get_double(doc, "costMember")?,
        get_double(doc, "costNonMember")?,
        get_integer(doc, "waitingList")?,
        get_integer(doc, "maxWaitingList")?,
        get_string(doc, "location")?,
        get_string(doc, "bookingTemplate")?,
        get_string(doc, "waitingTemplate")?,
        get_opt_string(doc, "altBookingButtnText")?,
        get_opt_string(doc, "altEmailAddress")?,
        get_bool(doc, "externalOperator")?,
    );
    Ok(event)
}

fn to_subscription(doc: &mut Document) -> Result<Subscription> {
    let index = doc
        .name
        .rfind('/')
        .with_context(|| format!("Found no / in document name {}", doc.name))?;
    let email: &str = &doc.name[index + 1..];

    let event = Subscription::new(
        email.into(),
        get_strings(doc, "types")?
            .iter()
            .map(|s| NewsType::from_str(s))
            .collect::<Result<Vec<_>, _>>()?,
    );
    Ok(event)
}

fn get_string(doc: &mut Document, key: &str) -> Result<String> {
    let value_type = extract_value_type_(doc, key)?;
    if let ValueType::StringValue(value) = value_type {
        return Ok(value);
    }
    bail!(
        "Field {} in document {} has wrong value type ({:?})",
        key,
        doc.name,
        value_type
    )
}

fn get_integer(doc: &mut Document, key: &str) -> Result<i64> {
    let value_type = extract_value_type_(doc, key)?;
    if let ValueType::IntegerValue(value) = value_type {
        return Ok(value);
    }
    bail!(
        "Field {} in document {} has wrong value type ({:?})",
        key,
        doc.name,
        value_type
    )
}

fn get_double(doc: &mut Document, key: &str) -> Result<f64> {
    let value_type = extract_value_type_(doc, key)?;
    if let ValueType::DoubleValue(value) = value_type {
        return Ok(value);
    }
    bail!(
        "Field {} in document {} has wrong value type ({:?})",
        key,
        doc.name,
        value_type
    )
}

fn get_bool(doc: &mut Document, key: &str) -> Result<bool> {
    let value_type = extract_value_type_(doc, key)?;
    if let ValueType::BooleanValue(value) = value_type {
        return Ok(value);
    }
    bail!(
        "Field {} in document {} has wrong value type ({:?})",
        key,
        doc.name,
        value_type
    )
}

fn get_strings(doc: &mut Document, key: &str) -> Result<Vec<String>> {
    let value_type = extract_value_type_(doc, key)?;
    if let ValueType::ArrayValue(ArrayValue { values }) = value_type {
        let mut strings: Vec<String> = Vec::new();
        for value in values.into_iter() {
            if let ValueType::StringValue(value) = value.value_type.ok_or_else(|| {
                anyhow!("Child of field {} in document {} is empty", key, doc.name)
            })? {
                strings.push(value);
            }
        }
        return Ok(strings);
    }
    bail!(
        "Field {} in document {} has wrong value type ({:?})",
        key,
        doc.name,
        value_type
    )
}

fn get_opt_string(doc: &mut Document, key: &str) -> Result<Option<String>> {
    if let Some(Value {
        value_type: Some(value_type),
    }) = doc.fields.remove(key)
    {
        return match value_type {
            ValueType::StringValue(value) => {
                if value.trim().is_empty() {
                    return Ok(None);
                }
                Ok(Some(value))
            }
            ValueType::NullValue(_) => Ok(None),
            _ => bail!(
                "Field {} in document {} has wrong value type ({:?})",
                key,
                doc.name,
                value_type
            ),
        };
    }
    Ok(None)
}

fn extract_value_type_(doc: &mut Document, key: &str) -> Result<ValueType> {
    let value_type = doc
        .fields
        .remove(key)
        .ok_or_else(|| anyhow!("Field {} is missing in document {}", key, doc.name))?
        .value_type
        .ok_or_else(|| anyhow!("Field {} is missing in document {}", key, doc.name))?;
    Ok(value_type)
}
