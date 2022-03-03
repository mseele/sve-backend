use crate::logic::{calendar, contact, events, news};
use crate::models::{ContactMessage, EventBooking, MassEmails, PartialEvent, Subscription};
use actix_web::http::header::ContentType;
use actix_web::{error, HttpRequest, HttpResponseBuilder};
use actix_web::{http::header, http::StatusCode};
use actix_web::{web, HttpResponse, Responder, Result};
use log::error;
use serde::Deserialize;
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
        write!(f, "{}", self.err)
    }
}

impl Display for ResponseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.err)
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
            .route("/delete", web::post().to(delete)),
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
}

// events

#[derive(Debug, Deserialize)]
pub struct EventsRequest {
    all: Option<bool>,
    beta: Option<bool>,
}

async fn events(info: web::Query<EventsRequest>) -> Result<impl Responder, ResponseError> {
    let events = events::get_events(info.all, info.beta).await?;
    Ok(web::Json(events))
}

async fn counter() -> Result<impl Responder, ResponseError> {
    let event_counters = events::get_event_counters().await?;
    Ok(web::Json(event_counters))
}

async fn booking(booking: web::Json<EventBooking>) -> Result<impl Responder, ResponseError> {
    let response = events::booking(booking.0).await;
    Ok(web::Json(response))
}

async fn prebooking(hash: String) -> Result<impl Responder, ResponseError> {
    let response = events::prebooking(hash).await;
    Ok(web::Json(response))
}

async fn update(partial_event: web::Json<PartialEvent>) -> Result<impl Responder, ResponseError> {
    let event = events::update(partial_event.0).await?;
    Ok(web::Json(event))
}

async fn delete(partial_event: web::Json<PartialEvent>) -> Result<impl Responder, ResponseError> {
    events::delete(partial_event.0).await?;
    Ok(HttpResponse::Ok().finish())
}

// news

async fn subscribe(subscription: web::Json<Subscription>) -> Result<impl Responder, ResponseError> {
    news::subscribe(subscription.0).await?;
    Ok(HttpResponse::Ok().finish())
}

async fn unsubscribe(
    subscription: web::Json<Subscription>,
) -> Result<impl Responder, ResponseError> {
    news::unsubscribe(subscription.0).await?;
    Ok(HttpResponse::Ok().finish())
}

async fn subscribers() -> Result<impl Responder, ResponseError> {
    let subscriptions = news::get_subscriptions().await?;
    let result = subscriptions
        .into_iter()
        .map(|(news_type, emails)| {
            let title: &str = &format!(
                "---------- {}: {} ----------",
                news_type.display_name(),
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
    Ok(web::Json(result))
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

async fn message(message: web::Json<ContactMessage>) -> Result<impl Responder, ResponseError> {
    contact::message(message.0).await?;
    Ok(HttpResponse::Ok().finish())
}

async fn emails(emails: web::Json<MassEmails>) -> Result<impl Responder, ResponseError> {
    contact::emails(emails.0.emails).await?;
    Ok(HttpResponse::Ok().finish())
}
