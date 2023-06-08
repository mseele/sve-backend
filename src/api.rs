use crate::logic::{calendar, contact, events, export, news, tasks};
use crate::models::{
    ContactMessage, Email, EventBooking, EventEmail, EventId, EventType, LifecycleStatus,
    NewsSubscription, PartialEvent,
};
use axum::extract::{self, Path, Query, State};
use axum::http::{
    header::{self, HeaderMap},
    StatusCode,
};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, patch, post};
use axum::{Json, Router};
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

impl IntoResponse for ResponseError {
    fn into_response(self) -> Response {
        error!("{:?}", self);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            [(header::CONTENT_TYPE, "text/plain")],
            "An internal error occurred. Please try again later.",
        ).into_response()
    }
}

pub(crate) fn router(state: PgPool) -> Router {
    Router::new()
        .nest(
            "/api",
            Router::new()
                .nest(
                    "/events",
                    Router::new()
                        .route("/", get(events))
                        .route("/counter", get(counter))
                        .route("/booking", post(booking))
                        .route("/prebooking/:hash", get(prebooking))
                        .route("/update", post(update))
                        .route("/:id", delete(delete_event))
                        .nest(
                            "/booking",
                            Router::new()
                                .route(
                                    "/:id",
                                    patch(update_event_booking).delete(cancel_event_booking),
                                )
                                .route("/export/:event_id", get(export_event_bookings))
                                .route(
                                    "/participants_list/:event_id",
                                    get(export_event_participants_list),
                                ),
                        )
                        .nest(
                            "/payments",
                            Router::new()
                                .route("/verify", post(verify_payments))
                                .route("/unpaid/:event_type", get(unpaid_bookings)),
                        ),
                )
                .nest(
                    "/news",
                    Router::new()
                        .route("/subscribe", post(subscribe))
                        .route("/unsubscribe", post(unsubscribe))
                        .route("/subscribers", get(subscribers)),
                )
                .nest(
                    "/contact",
                    Router::new()
                        .route("/message", post(message))
                        .route("/emails", post(emails)),
                )
                .nest(
                    "/calendar",
                    Router::new()
                        .route("/appointments", get(appointments))
                        .route("/notifications", post(notifications)),
                )
                .nest(
                    "/tasks",
                    Router::new()
                        .route("/check_email_connectivity", get(check_email_connectivity))
                        .route("/renew_calendar_watch", get(renew_calendar_watch))
                        .route("/send_event_reminders", get(send_event_reminders))
                        .route("/close_finished_events", get(close_finished_events))
                        .route(
                            "/send_payment_reminders/:event_type",
                            get(send_payment_reminders),
                        )
                        .route(
                            "/send_participation_confirmation/:event_id",
                            get(send_participation_confirmation),
                        ),
                ),
        )
        .with_state(state)
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
    State(pool): State<PgPool>,
    mut query: Query<EventsQueryParams>,
) -> Result<impl IntoResponse, ResponseError> {
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
    State(pool): State<PgPool>,
    query: Query<EventCountersQueryParams>,
) -> Result<impl IntoResponse, ResponseError> {
    let event_counters = events::get_event_counters(&pool, query.beta).await?;
    Ok(Json(event_counters))
}

async fn booking(
    State(pool): State<PgPool>,
    extract::Json(booking): extract::Json<EventBooking>,
) -> Result<impl IntoResponse, ResponseError> {
    let response = events::booking(&pool, booking).await;
    Ok(Json(response))
}

async fn prebooking(
    State(pool): State<PgPool>,
    Path(hash): Path<String>,
) -> Result<impl IntoResponse, ResponseError> {
    let response = events::prebooking(&pool, hash).await;
    Ok(Json(response))
}

async fn update(
    State(pool): State<PgPool>,
    extract::Json(partial_event): extract::Json<PartialEvent>,
) -> Result<impl IntoResponse, ResponseError> {
    let event = events::update(&pool, partial_event).await?;
    Ok(Json(event))
}

async fn delete_event(
    State(pool): State<PgPool>,
    Path(path): Path<EventId>,
) -> Result<impl IntoResponse, ResponseError> {
    events::delete(&pool, path).await?;
    Ok(StatusCode::OK)
}

async fn verify_payments(
    State(pool): State<PgPool>,
    extract::Json(input): extract::Json<VerifyPaymentInput>,
) -> Result<impl IntoResponse, ResponseError> {
    Ok(Json(
        events::verify_payments(&pool, input.csv, input.start_date).await?,
    ))
}

async fn unpaid_bookings(
    State(pool): State<PgPool>,
    Path(event_type): Path<EventType>,
) -> Result<impl IntoResponse, ResponseError> {
    Ok(Json(events::get_unpaid_bookings(&pool, event_type).await?))
}

async fn update_event_booking(
    State(pool): State<PgPool>,
    Path(booking_id): Path<i32>,
    query: Query<UpdateEventBookingQueryParams>,
) -> Result<impl IntoResponse, ResponseError> {
    if let Some(update_payment) = query.update_payment {
        events::update_payment(&pool, booking_id, update_payment).await?;
    }
    Ok(StatusCode::OK)
}

async fn cancel_event_booking(
    State(pool): State<PgPool>,
    Path(booking_id): Path<i32>,
) -> Result<impl IntoResponse, ResponseError> {
    events::cancel_booking(&pool, booking_id).await?;
    Ok(StatusCode::OK)
}

async fn export_event_bookings(
    State(pool): State<PgPool>,
    Path(event_id): Path<EventId>,
) -> Result<impl IntoResponse, ResponseError> {
    let (filename, bytes) = export::event_bookings(&pool, event_id).await?;
    Ok(into_file_response(filename, bytes))
}

async fn export_event_participants_list(
    State(pool): State<PgPool>,
    Path(event_id): Path<EventId>,
) -> Result<impl IntoResponse, ResponseError> {
    let (filename, bytes) = export::event_participants_list(&pool, event_id).await?;
    Ok(into_file_response(filename, bytes))
}

// news

async fn subscribe(
    State(pool): State<PgPool>,
    extract::Json(subscription): extract::Json<NewsSubscription>,
) -> Result<impl IntoResponse, ResponseError> {
    news::subscribe(&pool, subscription).await?;
    Ok(StatusCode::OK)
}

async fn unsubscribe(
    State(pool): State<PgPool>,
    extract::Json(subscription): extract::Json<NewsSubscription>,
) -> Result<impl IntoResponse, ResponseError> {
    news::unsubscribe(&pool, subscription).await?;
    Ok(StatusCode::OK)
}

async fn subscribers(State(pool): State<PgPool>) -> Result<impl IntoResponse, ResponseError> {
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

    Ok((
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/html")],
        result,
    ))
}

// calendar

async fn appointments() -> Result<impl IntoResponse, ResponseError> {
    let result = calendar::appointments().await?;
    Ok(Json(result))
}

async fn notifications(headers: HeaderMap) -> Result<impl IntoResponse, ResponseError> {
    let header_key = "X-Goog-Channel-Id";
    let channel_id = headers.get(header_key);
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
    Ok(StatusCode::OK)
}

// contact

#[derive(Deserialize, Debug)]
struct EmailsBody {
    emails: Option<Vec<Email>>,
    event: Option<EventEmail>,
}

async fn message(Json(message): Json<ContactMessage>) -> Result<impl IntoResponse, ResponseError> {
    contact::message(message).await?;
    Ok(StatusCode::OK)
}

async fn emails(
    State(pool): State<PgPool>,
    extract::Json(body): extract::Json<EmailsBody>,
) -> Result<impl IntoResponse, ResponseError> {
    if let Some(emails) = body.emails {
        contact::emails(emails).await?;
    } else if let Some(event) = body.event {
        events::send_event_email(&pool, event).await?;
    }
    Ok(StatusCode::OK)
}

// tasks

async fn check_email_connectivity() -> Result<impl IntoResponse, ResponseError> {
    tasks::check_email_connectivity().await;
    Ok(StatusCode::OK)
}

async fn renew_calendar_watch() -> Result<impl IntoResponse, ResponseError> {
    tasks::renew_calendar_watch().await;
    Ok(StatusCode::OK)
}

async fn send_event_reminders(
    State(pool): State<PgPool>,
) -> Result<impl IntoResponse, ResponseError> {
    tasks::send_event_reminders(&pool).await;
    Ok(StatusCode::OK)
}

async fn close_finished_events(
    State(pool): State<PgPool>,
) -> Result<impl IntoResponse, ResponseError> {
    tasks::close_finished_events(&pool).await;
    Ok(StatusCode::OK)
}

async fn send_payment_reminders(
    State(pool): State<PgPool>,
    Path(event_type): Path<EventType>,
) -> Result<impl IntoResponse, ResponseError> {
    tasks::send_payment_reminders(&pool, event_type).await?;
    Ok(StatusCode::OK)
}

async fn send_participation_confirmation(
    State(pool): State<PgPool>,
    Path(event_id): Path<EventId>,
) -> Result<impl IntoResponse, ResponseError> {
    tasks::send_participation_confirmation(&pool, event_id).await?;
    Ok(StatusCode::OK)
}

fn into_file_response(filename: String, bytes: Vec<u8>) -> impl IntoResponse {
    (
        StatusCode::OK,
        [(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{filename}\""),
        )],
        bytes,
    )
}
