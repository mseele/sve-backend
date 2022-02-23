use crate::models::{EventCounter, PartialEvent};
use crate::store::{self, GouthInterceptor};
use crate::{api::ResponseError, models::Event};
use actix_web::{web, HttpResponse, Responder, Result};
use googapis::google::firestore::v1::firestore_client::FirestoreClient;
use serde::Deserialize;
use std::fmt::Debug;
use tonic::codegen::InterceptedService;
use tonic::transport::Channel;

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/events")
            .route("", web::get().to(events))
            .route("/counter", web::get().to(counter))
            // TODO: .route("/booking", web::post().to(booking))
            // TODO: .route("/prebooking", web::post().to(prebooking))
            .route("/update", web::post().to(update))
            .route("/delete", web::post().to(delete)),
    );
}

#[derive(Debug, Deserialize)]
pub struct EventsRequest {
    all: Option<bool>,
    beta: Option<bool>,
}

// handlers

async fn events(info: web::Query<EventsRequest>) -> Result<impl Responder, ResponseError> {
    let mut client = store::get_client().await?;
    let mut events = get_events(&mut client, info.all, info.beta).await?;

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

async fn counter() -> Result<impl Responder, ResponseError> {
    let mut client = store::get_client().await?;
    let event_counters = get_event_counters(&mut client).await?;

    Ok(web::Json(event_counters))
}

async fn update(partial_event: web::Json<PartialEvent>) -> Result<impl Responder, ResponseError> {
    let partial_event = partial_event.0;
    let mut client = store::get_client().await?;
    let result = store::write_event(&mut client, partial_event).await?;

    Ok(web::Json(result))
}

async fn delete(partial_event: web::Json<PartialEvent>) -> Result<HttpResponse, ResponseError> {
    let mut client = store::get_client().await?;
    store::delete_event(&mut client, &partial_event.0.id).await?;

    Ok(HttpResponse::Ok().finish())
}

// logic

async fn get_events(
    client: &mut FirestoreClient<InterceptedService<Channel, GouthInterceptor>>,
    all: Option<bool>,
    beta: Option<bool>,
) -> anyhow::Result<Vec<Event>> {
    let values = store::get_events(client).await?;

    let values = values
        .into_iter()
        .filter(|event| {
            // keep event if all is true
            if let Some(all) = all {
                if all {
                    return true;
                }
            }
            // keep event if it is visible
            if event.visible {
                // and beta is the same state than event
                if let Some(beta) = beta {
                    return beta == event.beta;
                }
                // and beta is None
                return true;
            }
            return false;
        })
        .collect::<Vec<_>>();

    Ok(values)
}

async fn get_event_counters(
    client: &mut FirestoreClient<InterceptedService<Channel, GouthInterceptor>>,
) -> anyhow::Result<Vec<EventCounter>> {
    let event_counters = get_events(client, None, None)
        .await?
        .into_iter()
        .map(|event| event.into())
        .collect::<Vec<EventCounter>>();

    Ok(event_counters)
}
