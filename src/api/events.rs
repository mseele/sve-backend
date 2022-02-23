use crate::api::ResponseError;
use crate::store;
use actix_web::{get, web, Responder, Result};
use serde::Deserialize;
use std::fmt::Debug;

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

    let mut client = store::get_client().await?;
    let events = store::get_events(&mut client).await?;

    Ok(web::Json(events))
}
