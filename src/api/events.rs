use crate::store;
use actix_web::{error, get, web, HttpResponseBuilder, Responder, Result};
use actix_web::{http::header, http::StatusCode, HttpResponse};
use serde::Deserialize;
use std::error::Error;
use std::fmt::{Debug, Display};

struct ResponseError {
    err: anyhow::Error,
}

impl Error for ResponseError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.err.source()
    }

    // fn backtrace(&self) -> Option<&std::backtrace::Backtrace> {
    //    self.err.backtrace()
    // }

    fn description(&self) -> &str {
        self.err.description()
    }

    fn cause(&self) -> Option<&dyn Error> {
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
    cfg.service(web::scope("/events").service(events));
}

#[derive(Debug, Deserialize)]
pub struct EventsRequest {
    all: Option<bool>,
    beta: Option<bool>,
}

#[get("")]
async fn events(info: web::Query<EventsRequest>) -> Result<impl Responder, ResponseError> {
    //TODO: work with info

    let mut client = store::events::get_client().await?;
    let events = store::events::get_events(&mut client).await?;

    Ok(web::Json(events))
}
