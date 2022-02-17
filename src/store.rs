use crate::models::{Event, EventType, NewsType, PartialEvent, Subscription};
use anyhow::{bail, Context, Result};
use chrono::NaiveDateTime;
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

const PROJECT: &str = include_str!("../data/project.id");

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

    let mut events: Vec<Event> = Vec::new();

    // TODO: use into_inner() to avoid borrowing (and clone)
    for document in &response.get_ref().documents {
        events.push(to_event(&document)?);
    }

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

    // TODO: use into_inner() to avoid borrowing (and clone)
    let event = to_event(&response.get_ref())?;

    Ok(event)
}

pub async fn write_event(
    client: &mut FirestoreClient<InterceptedService<Channel, GouthInterceptor>>,
    partial_event: PartialEvent,
) -> Result<()> {
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
    let document_result = document_result?;
    if document_result.is_some() {
        update_event(client, partial_event).await?;
    } else {
        create_event(client, partial_event.try_into()?).await?;
    }

    Ok(())
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
    Booked(Event),
    WaitingList(Event),
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
                BookingTransaction::Book(writes) => {
                    // commit transaction
                    commit_transaction(client, writes, transaction).await?;
                    let event = get_event(client, id).await?;
                    Ok(BookingResult::Booked(event))
                }
                BookingTransaction::WaitingList(writes) => {
                    // commit transaction
                    commit_transaction(client, writes, transaction).await?;
                    let event = get_event(client, id).await?;
                    Ok(BookingResult::WaitingList(event))
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
    Book(Vec<Write>),
    WaitingList(Vec<Write>),
    BookedOut,
}

pub async fn run_book_event_transaction(
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
            consistency_selector: Some(ConsistencySelector::Transaction(transaction)),
            ..Default::default()
        }))
        .await?;

    // extract and increment counters
    // TODO: use into_inner() to avoid borrowing (and clone)
    let doc = response.get_ref();
    let subscribers = get_integer(doc, subscribers_field)?;
    let max_subscribers = get_integer(doc, max_subscribers_field)?;
    if max_subscribers == -1 || subscribers < max_subscribers {
        return Ok(BookingTransaction::Book(vec![Write {
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
        }]));
    }

    let waiting_list = get_integer(doc, waiting_list_field)?;
    let max_waiting_list = get_integer(doc, max_waiting_list_field)?;
    if waiting_list < max_waiting_list {
        return Ok(BookingTransaction::WaitingList(vec![Write {
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
        }]));
    }

    Ok(BookingTransaction::BookedOut)
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

        // TODO: avoid borrowing (and clone)
        for document in response.documents {
            subscriptions.push(to_subscription(&document)?);
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
        Ok(response) => Ok(Some(to_subscription(&response.get_ref())?)),
        Err(status) if status.code() == Code::NotFound => Ok(None),
        Err(status) => Err(status.into()),
    }
}

async fn create_event(
    client: &mut FirestoreClient<InterceptedService<Channel, GouthInterceptor>>,
    event: Event,
) -> Result<()> {
    client
        .create_document(Request::new(CreateDocumentRequest {
            parent: format!("projects/{}/databases/(default)/documents", PROJECT),
            collection_id: "events".into(),
            document_id: event.id.clone(),
            document: Some(from_event(event)),
            ..Default::default()
        }))
        .await?;

    Ok(())
}

async fn update_event(
    client: &mut FirestoreClient<InterceptedService<Channel, GouthInterceptor>>,
    partial_event: PartialEvent,
) -> Result<()> {
    let (document, update_mask) = from_partial_event(partial_event);
    client
        .update_document(Request::new(UpdateDocumentRequest {
            document: Some(document),
            update_mask: Some(update_mask),
            ..Default::default()
        }))
        .await?;

    Ok(())
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

fn to_event(doc: &Document) -> Result<Event> {
    let name = &doc.name;
    let index = name
        .rfind('/')
        .with_context(|| format!("Found no / in document name {}", doc.name))?;
    let id: &str = &name[index + 1..];

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

fn to_subscription(doc: &Document) -> Result<Subscription> {
    let name = &doc.name;
    let index = name
        .rfind('/')
        .with_context(|| format!("Found no / in document name {}", doc.name))?;
    let email: &str = &name[index + 1..];

    let event = Subscription::new(
        email.into(),
        get_strings(doc, "types")?
            .iter()
            .map(|s| NewsType::from_str(s))
            .collect::<Result<Vec<_>, _>>()?,
    );
    Ok(event)
}

fn get_string(doc: &Document, key: &str) -> Result<String> {
    match extract_value_type(doc, key, true)? {
        Some(value_type) => match value_type {
            ValueType::StringValue(value) => Ok(value.clone()),
            _ => bail!(
                "Field {} in document {} has wrong value type ({:?})",
                key,
                doc.name,
                value_type
            ),
        },
        None => bail!("Failed to extract field {} from document {}", key, doc.name),
    }
}

fn get_integer(doc: &Document, key: &str) -> Result<i64> {
    match extract_value_type(doc, key, true)? {
        Some(value_type) => match value_type {
            ValueType::IntegerValue(value) => Ok(value.clone()),
            _ => bail!(
                "Field {} in document {} has wrong value type ({:?})",
                key,
                doc.name,
                value_type
            ),
        },
        None => bail!("Failed to extract field {} from document {}", key, doc.name),
    }
}

fn get_double(doc: &Document, key: &str) -> Result<f64> {
    match extract_value_type(doc, key, true)? {
        Some(value_type) => match value_type {
            ValueType::DoubleValue(value) => Ok(value.clone()),
            _ => bail!(
                "Field {} in document {} has wrong value type ({:?})",
                key,
                doc.name,
                value_type
            ),
        },
        None => bail!("Failed to extract field {} from document {}", key, doc.name),
    }
}

fn get_bool(doc: &Document, key: &str) -> Result<bool> {
    match extract_value_type(doc, key, true)? {
        Some(value_type) => match value_type {
            ValueType::BooleanValue(value) => Ok(value.clone()),
            _ => bail!(
                "Field {} in document {} has wrong value type ({:?})",
                key,
                doc.name,
                value_type
            ),
        },
        None => bail!("Failed to extract field {} from document {}", key, doc.name),
    }
}

fn get_opt_string(doc: &Document, key: &str) -> Result<Option<String>> {
    match extract_value_type(doc, key, false)? {
        Some(value_type) => match value_type {
            ValueType::StringValue(value) => {
                if value.trim().is_empty() {
                    return Ok(None);
                }
                Ok(Some(value.clone()))
            }
            ValueType::NullValue(_) => Ok(None),
            _ => bail!(
                "Field {} in document {} has wrong value type ({:?})",
                key,
                doc.name,
                value_type
            ),
        },
        None => Ok(None),
    }
}

fn get_strings(doc: &Document, key: &str) -> Result<Vec<String>> {
    match extract_value_type(doc, key, true)? {
        Some(value_type) => match value_type {
            ValueType::ArrayValue(value) => {
                let mut vec: Vec<String> = Vec::new();
                for value in value.values.iter() {
                    match &value.value_type {
                        Some(v_value_type) => match v_value_type {
                            ValueType::StringValue(v) => {
                                vec.push(v.clone());
                            }
                            _ => bail!(
                                "Child of field {} in document {} has wrong value type ({:?})",
                                key,
                                doc.name,
                                v_value_type
                            ),
                        },
                        None => bail!("Child of field {} in document {} is empty", key, doc.name),
                    }
                }
                Ok(vec)
            }
            _ => bail!(
                "Field {} in document {} has wrong value type ({:?})",
                key,
                doc.name,
                value_type
            ),
        },
        None => bail!("Failed to extract field {} from document {}", key, doc.name),
    }
}

fn extract_value_type<'a>(
    doc: &'a Document,
    key: &str,
    required: bool,
) -> Result<&'a Option<ValueType>> {
    match doc.fields.get(key).map(|v| &v.value_type) {
        Some(value_type) => Ok(value_type),
        None => {
            if required {
                bail!("Field {} is missing in document {}", key, doc.name);
            }
            return Ok(&None);
        }
    }
}
