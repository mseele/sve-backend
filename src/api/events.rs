use crate::store;
use crate::{api::ResponseError, models::Event};
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
    let mut client = store::get_client().await?;
    let events = store::get_events(&mut client).await?;

    let mut events = events
        .into_iter()
        .filter(|event| {
            // keep event if all is true
            if let Some(all) = info.all {
                if all {
                    return true;
                }
            }
            // keep event if it is visible
            if event.visible {
                // and beta is the same state than event
                if let Some(beta) = info.beta {
                    return beta == event.beta;
                }
                // and beta is None
                return true;
            }
            return false;
        })
        .collect::<Vec<_>>();

    // sort the events
    events.sort_unstable_by(|a, b| {
        let is_a_booked_up = a.is_booked_up();
        let is_b_booked_up = b.is_booked_up();
        if is_a_booked_up == is_b_booked_up {
            return a.sort_index.cmp(&b.sort_index);
        }
        return is_a_booked_up.cmp(&is_b_booked_up);
    });

    Ok(web::Json(events))
}
