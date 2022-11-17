use crate::logic::{calendar, contact, events, export, news, tasks};
use crate::models::{
    ContactMessage, Email, EventBooking, EventEmail, EventId, EventType, LifecycleStatus,
    NewsSubscription, PartialEvent,
};
use actix_web::http::header::{ContentDisposition, ContentType, DispositionParam, DispositionType};
use actix_web::web::{Data, Json};
use actix_web::{error, HttpRequest, HttpResponseBuilder};
use actix_web::{http::header, http::StatusCode};
use actix_web::{web, HttpResponse, Responder, Result};
use chrono::NaiveDate;
use log::error;
use serde::de;
use serde::Deserialize;
use sqlx::PgPool;
use std::error::Error;
use std::fmt::Debug;
use std::fmt::{self, Display};
use std::str::FromStr;

pub(crate) struct ResponseError {
    err: anyhow::Error,
}

impl Error for ResponseError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.err.source()
    }

    //TODO: what to do with backtrace
    // fn backtrace(&self) -> Option<&std::backtrace::Backtrace> {
    //    self.err.backtrace()
    // }

    fn description(&self) -> &str {
        #[allow(deprecated)]
        self.err.description()
    }

    fn cause(&self) -> Option<&dyn Error> {
        #[allow(deprecated)]
        self.err.cause()
    }
}

impl Debug for ResponseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.err)
    }
}

impl Display for ResponseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.err)
    }
}

impl From<anyhow::Error> for ResponseError {
    fn from(err: anyhow::Error) -> ResponseError {
        ResponseError { err }
    }
}

impl error::ResponseError for ResponseError {
    fn error_response(&self) -> HttpResponse {
        HttpResponseBuilder::new(self.status_code())
            .insert_header(header::ContentType(mime::TEXT_PLAIN_UTF_8))
            .body("An internal error occurred. Please try again later.")
    }

    fn status_code(&self) -> StatusCode {
        StatusCode::INTERNAL_SERVER_ERROR
    }
}

pub(crate) fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/events")
            .route("", web::get().to(events))
            .route("/counter", web::get().to(counter))
            .route("/booking", web::post().to(booking))
            .route("/prebooking/{hash}", web::get().to(prebooking))
            .route("/update", web::post().to(update))
            .route("/{id}", web::delete().to(delete))
            .route("/booking/{id}", web::patch().to(update_event_booking))
            .route("/booking/{id}", web::delete().to(cancel_event_booking))
            .route(
                "/booking/export/{event_id}",
                web::get().to(export_event_bookings),
            )
            .route("/payments/verify", web::post().to(verify_payments))
            .route(
                "/payments/unpaid/{event_type}",
                web::get().to(unpaid_bookings),
            ),
    );
    cfg.service(
        web::scope("/news")
            .route("/subscribe", web::post().to(subscribe))
            .route("/unsubscribe", web::post().to(unsubscribe))
            .route("/subscribers", web::get().to(subscribers)),
    );
    cfg.service(
        web::scope("/contact")
            .route("/message", web::post().to(message))
            .route("/emails", web::post().to(emails)),
    );
    cfg.service(
        web::scope("/calendar")
            .route("/appointments", web::get().to(appointments))
            .route("/notifications", web::post().to(notifications)),
    );
    cfg.service(
        web::scope("/tasks")
            .route(
                "/check_email_connectivity",
                web::get().to(check_email_connectivity),
            )
            .route("/renew_calendar_watch", web::get().to(renew_calendar_watch))
            .route("/send_event_reminders", web::get().to(send_event_reminders))
            .route(
                "/send_payment_reminders/{event_type}",
                web::get().to(send_payment_reminders),
            ),
    );
}

// events

pub(crate) fn deserialize_lifecycle_status_list<'de, D>(
    deserializer: D,
) -> Result<Option<Vec<LifecycleStatus>>, D::Error>
where
    D: de::Deserializer<'de>,
{
    struct StringVecVisitor;

    impl<'de> de::Visitor<'de> for StringVecVisitor {
        type Value = Option<Vec<LifecycleStatus>>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter
                .write_str("a string containing a comma separated list of lifecycle status strings")
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            let mut list = Vec::new();
            for status in v.split(',') {
                list.push(LifecycleStatus::from_str(status).map_err(E::custom)?);
            }
            Ok(match list.len() {
                0 => None,
                _ => Some(list),
            })
        }
    }

    deserializer.deserialize_any(StringVecVisitor)
}

#[derive(Debug, Deserialize)]
struct EventsQueryParams {
    beta: Option<bool>,
    #[serde(default, deserialize_with = "deserialize_lifecycle_status_list")]
    status: Option<Vec<LifecycleStatus>>,
    subscribers: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct EventCountersQueryParams {
    beta: bool,
}

#[derive(Debug, Deserialize)]
pub(crate) struct VerifyPaymentInput {
    csv: String,
    start_date: Option<NaiveDate>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateEventBookingQueryParams {
    update_payment: Option<bool>,
}

async fn events(
    pool: Data<PgPool>,
    mut query: web::Query<EventsQueryParams>,
) -> Result<impl Responder, ResponseError> {
    let events = events::get_events(
        &pool,
        query.beta.take(),
        query.status.take(),
        query.subscribers.take(),
    )
    .await?;
    Ok(Json(events))
}

async fn counter(
    pool: Data<PgPool>,
    query: web::Query<EventCountersQueryParams>,
) -> Result<impl Responder, ResponseError> {
    let event_counters = events::get_event_counters(&pool, query.beta).await?;
    Ok(Json(event_counters))
}

async fn booking(
    pool: Data<PgPool>,
    Json(booking): Json<EventBooking>,
) -> Result<impl Responder, ResponseError> {
    let response = events::booking(&pool, booking).await;
    Ok(Json(response))
}

async fn prebooking(
    pool: Data<PgPool>,
    path: web::Path<String>,
) -> Result<impl Responder, ResponseError> {
    let response = events::prebooking(&pool, path.into_inner()).await;
    Ok(Json(response))
}

async fn update(
    pool: Data<PgPool>,
    Json(partial_event): Json<PartialEvent>,
) -> Result<impl Responder, ResponseError> {
    let event = events::update(&pool, partial_event).await?;
    Ok(Json(event))
}

async fn delete(
    pool: Data<PgPool>,
    path: web::Path<EventId>,
) -> Result<impl Responder, ResponseError> {
    events::delete(&pool, path.into_inner()).await?;
    Ok(HttpResponse::Ok().finish())
}

async fn verify_payments(
    pool: Data<PgPool>,
    Json(input): Json<VerifyPaymentInput>,
) -> Result<impl Responder, ResponseError> {
    Ok(Json(
        events::verify_payments(&pool, input.csv, input.start_date).await?,
    ))
}

async fn unpaid_bookings(
    pool: Data<PgPool>,
    path: web::Path<EventType>,
) -> Result<impl Responder, ResponseError> {
    Ok(Json(
        events::get_unpaid_bookings(&pool, path.into_inner()).await?,
    ))
}

async fn update_event_booking(
    pool: Data<PgPool>,
    path: web::Path<i32>,
    query: web::Query<UpdateEventBookingQueryParams>,
) -> Result<impl Responder, ResponseError> {
    let booking_id = path.into_inner();
    if let Some(update_payment) = query.update_payment {
        events::update_payment(&pool, booking_id, update_payment).await?;
    }
    Ok(HttpResponse::Ok().finish())
}

async fn cancel_event_booking(
    pool: Data<PgPool>,
    path: web::Path<i32>,
) -> Result<impl Responder, ResponseError> {
    let booking_id = path.into_inner();
    events::cancel_booking(&pool, booking_id).await?;
    Ok(HttpResponse::Ok().finish())
}

async fn export_event_bookings(
    pool: Data<PgPool>,
    path: web::Path<EventId>,
) -> Result<impl Responder, ResponseError> {
    let event_id = path.into_inner();
    let (filename, bytes) = export::event_bookings(&pool, event_id).await?;

    Ok(HttpResponse::Ok()
        .content_type(ContentType::octet_stream())
        .insert_header(ContentDisposition {
            disposition: DispositionType::Attachment,
            parameters: vec![DispositionParam::Filename(filename)],
        })
        .body(bytes))
}

// news

async fn subscribe(
    pool: Data<PgPool>,
    Json(subscription): Json<NewsSubscription>,
) -> Result<impl Responder, ResponseError> {
    news::subscribe(&pool, subscription).await?;
    Ok(HttpResponse::Ok().finish())
}

async fn unsubscribe(
    pool: Data<PgPool>,
    Json(subscription): Json<NewsSubscription>,
) -> Result<impl Responder, ResponseError> {
    news::unsubscribe(&pool, subscription).await?;
    Ok(HttpResponse::Ok().finish())
}

async fn subscribers(pool: Data<PgPool>) -> Result<impl Responder, ResponseError> {
    let subscriptions = news::get_subscriptions(&pool).await?;
    let result = subscriptions
        .into_iter()
        .map(|(topic, emails)| {
            let title: &str = &format!(
                "---------- {}: {} ----------",
                topic.display_name(),
                emails.len()
            );
            vec![
                title,
                &emails.into_iter().collect::<Vec<_>>().join(";"),
                title,
            ]
            .join("<br/>")
        })
        .collect::<Vec<_>>()
        .join("<br/><br/><br/>");

    Ok(HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(result))
}

// calendar

async fn appointments() -> Result<impl Responder, ResponseError> {
    let result = calendar::appointments().await?;
    Ok(Json(result))
}

async fn notifications(req: HttpRequest) -> Result<impl Responder, ResponseError> {
    let header_key = "X-Goog-Channel-Id";
    let channel_id = req.headers().get(header_key);
    if let Some(channel_id) = channel_id {
        match channel_id.to_str() {
            Ok(channel_id) => calendar::notifications(channel_id).await?,
            Err(e) => error!(
                "Could not parse header '{}' into a str: {:?}",
                header_key, e
            ),
        }
    } else {
        error!("Header '{}' has not been found in the request", header_key);
    }
    Ok(HttpResponse::Ok().finish())
}

// contact

#[derive(Deserialize, Debug)]
struct EmailsBody {
    emails: Option<Vec<Email>>,
    event: Option<EventEmail>,
}

async fn message(Json(message): Json<ContactMessage>) -> Result<impl Responder, ResponseError> {
    contact::message(message).await?;
    Ok(HttpResponse::Ok().finish())
}

async fn emails(
    pool: Data<PgPool>,
    Json(body): Json<EmailsBody>,
) -> Result<impl Responder, ResponseError> {
    if let Some(emails) = body.emails {
        contact::emails(emails).await?;
    } else if let Some(event) = body.event {
        events::send_event_email(&pool, event).await?;
    }
    Ok(HttpResponse::Ok().finish())
}

// tasks

async fn check_email_connectivity() -> Result<impl Responder, ResponseError> {
    tasks::check_email_connectivity().await;
    Ok(HttpResponse::Ok().finish())
}

async fn renew_calendar_watch() -> Result<impl Responder, ResponseError> {
    tasks::renew_calendar_watch().await;
    Ok(HttpResponse::Ok().finish())
}

async fn send_event_reminders(pool: Data<PgPool>) -> Result<impl Responder, ResponseError> {
    tasks::send_event_reminders(&pool).await;
    Ok(HttpResponse::Ok().finish())
}

async fn send_payment_reminders(
    pool: Data<PgPool>,
    path: web::Path<EventType>,
) -> Result<impl Responder, ResponseError> {
    tasks::send_payment_reminders(&pool, path.into_inner()).await?;
    Ok(HttpResponse::Ok().finish())
}
