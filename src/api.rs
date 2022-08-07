use crate::logic::{calendar, contact, events, news, tasks};
use crate::models::{ContactMessage, EventBooking, MassEmails, PartialEvent, NewsSubscription};
use actix_web::http::header::ContentType;
use actix_web::web::{Data, Json};
use actix_web::{error, HttpRequest, HttpResponseBuilder};
use actix_web::{http::header, http::StatusCode};
use actix_web::{web, HttpResponse, Responder, Result};
use chrono::NaiveDate;
use log::error;
use serde::Deserialize;
use sqlx::PgPool;
use std::error::Error;
use std::fmt::Debug;
use std::fmt::Display;

pub struct ResponseError {
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

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/events")
            .route("", web::get().to(events))
            .route("/counter", web::get().to(counter))
            .route("/booking", web::post().to(booking))
            .route("/prebooking", web::post().to(prebooking))
            .route("/update", web::post().to(update))
            .route("/delete", web::post().to(delete))
            .route("/verify_payments", web::post().to(verify_payments)),
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
            .route("/renew_calendar_watch", web::get().to(renew_calendar_watch)),
    );
}

// events

#[derive(Debug, Deserialize)]
pub struct EventsRequest {
    all: Option<bool>,
    beta: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct VerifyPaymentInput {
    sheet_id: String,
    csv: String,
    start_date: Option<NaiveDate>,
}

#[derive(Debug, Deserialize)]
pub struct PrebookingInput {
    hash: String,
}

async fn events(info: web::Query<EventsRequest>) -> Result<impl Responder, ResponseError> {
    let events = events::get_events(info.all, info.beta).await?;
    Ok(Json(events))
}

async fn counter() -> Result<impl Responder, ResponseError> {
    let event_counters = events::get_event_counters().await?;
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
    Json(input): Json<PrebookingInput>,
) -> Result<impl Responder, ResponseError> {
    let response = events::prebooking(&pool, input.hash).await;
    Ok(Json(response))
}

async fn update(Json(partial_event): Json<PartialEvent>) -> Result<impl Responder, ResponseError> {
    let event = events::update(partial_event).await?;
    Ok(Json(event))
}

async fn delete(Json(partial_event): Json<PartialEvent>) -> Result<impl Responder, ResponseError> {
    events::delete(partial_event).await?;
    Ok(HttpResponse::Ok().finish())
}

async fn verify_payments(
    Json(input): Json<VerifyPaymentInput>,
) -> Result<impl Responder, ResponseError> {
    let result = events::verify_payments(input.sheet_id, input.csv, input.start_date).await?;
    Ok(Json(result))
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

async fn message(Json(message): Json<ContactMessage>) -> Result<impl Responder, ResponseError> {
    contact::message(message).await?;
    Ok(HttpResponse::Ok().finish())
}

async fn emails(Json(emails): Json<MassEmails>) -> Result<impl Responder, ResponseError> {
    contact::emails(emails.emails).await?;
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
