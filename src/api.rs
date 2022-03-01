use crate::logic::{events, news};
use crate::models::{EventBooking, PartialEvent, Subscription};
use actix_web::http::header::ContentType;
use actix_web::{web, HttpResponse, Responder, Result};
use serde::Deserialize;
use std::fmt::Debug;

use actix_web::{error, HttpResponseBuilder};
use actix_web::{http::header, http::StatusCode};
use std::error::Error;
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
            // TODO: .route("/prebooking", web::post().to(prebooking))
            .route("/update", web::post().to(update))
            .route("/delete", web::post().to(delete)),
    );
    cfg.service(
        web::scope("/news")
            .route("/subscribe", web::post().to(subscribe))
            .route("/unsubscribe", web::post().to(unsubscribe))
            .route("/subscribers", web::get().to(subscribers)),
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

async fn update(partial_event: web::Json<PartialEvent>) -> Result<impl Responder, ResponseError> {
    let event = events::update(partial_event.0).await?;
    Ok(web::Json(event))
}

async fn delete(partial_event: web::Json<PartialEvent>) -> Result<impl Responder, ResponseError> {
    events::delete(partial_event.0).await?;
    Ok(HttpResponse::Ok().finish())
}

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
