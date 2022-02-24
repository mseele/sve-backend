use crate::models::{BookingResponse, EventBooking, EventCounter};
use crate::models::{Event, PartialEvent};
use crate::sheets;
use crate::store::{self, BookingResult, GouthInterceptor};
use anyhow::Result;
use googapis::google::firestore::v1::firestore_client::FirestoreClient;
use log::error;
use tonic::codegen::InterceptedService;
use tonic::transport::Channel;

const MESSAGE_FAIL: &str =
    "Leider ist etwas schief gelaufen. Bitte versuche es später noch einmal.";

pub async fn get_events(all: Option<bool>, beta: Option<bool>) -> anyhow::Result<Vec<Event>> {
    let mut client = store::get_client().await?;
    let mut events = get_and_filter_events(&mut client, all, beta).await?;

    // sort events
    events.sort_unstable_by(|a, b| {
        let is_a_booked_up = a.is_booked_up();
        let is_b_booked_up = b.is_booked_up();
        if is_a_booked_up == is_b_booked_up {
            return a.sort_index.cmp(&b.sort_index);
        }
        return is_a_booked_up.cmp(&is_b_booked_up);
    });

    Ok(events)
}

pub async fn get_event_counters() -> anyhow::Result<Vec<EventCounter>> {
    let mut client = store::get_client().await?;
    let event_counters = create_event_counters(&mut client).await?;

    Ok(event_counters)
}

pub async fn booking(booking: EventBooking) -> BookingResponse {
    match do_booking(booking).await {
        Ok(response) => response,
        Err(e) => {
            error!("Booking failed: {}", e);
            BookingResponse::failure(MESSAGE_FAIL)
        }
    }
}

pub async fn update(partial_event: PartialEvent) -> Result<Event> {
    let mut client = store::get_client().await?;
    let result = store::write_event(&mut client, partial_event).await?;

    Ok(result)
}

pub async fn delete(partial_event: PartialEvent) -> Result<()> {
    let mut client = store::get_client().await?;
    store::delete_event(&mut client, &partial_event.id).await?;

    Ok(())
}

async fn get_and_filter_events(
    client: &mut FirestoreClient<InterceptedService<Channel, GouthInterceptor>>,
    all: Option<bool>,
    beta: Option<bool>,
) -> Result<Vec<Event>> {
    let events = store::get_events(client).await?;

    let events = events
        .into_iter()
        .filter(|event| {
            // keep event if all is true
            if all.unwrap_or(false) {
                return true;
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

    Ok(events)
}

async fn create_event_counters(
    client: &mut FirestoreClient<InterceptedService<Channel, GouthInterceptor>>,
) -> anyhow::Result<Vec<EventCounter>> {
    let event_counters = get_and_filter_events(client, None, None)
        .await?
        .into_iter()
        .map(|event| event.into())
        .collect::<Vec<EventCounter>>();

    Ok(event_counters)
}

async fn do_booking(booking: EventBooking) -> anyhow::Result<BookingResponse> {
    let mut client = store::get_client().await?;
    let result = match store::book_event(&mut client, &booking.event_id).await? {
        BookingResult::Booked(event) => {
            sheets::save_booking(&booking, &event).await?;
            subscribe_to_updates(&mut client, &booking, &event).await?;
            // TODO: send mail
            let message = "Die Buchung war erfolgreich. Du bekommst in den nächsten Minuten eine Bestätigung per E-Mail.";
            BookingResponse::success(message, create_event_counters(&mut client).await?)
        }
        BookingResult::WaitingList(event) => {
            sheets::save_booking(&booking, &event).await?;
            subscribe_to_updates(&mut client, &booking, &event).await?;
            // TODO: send mail
            let message = "Du stehst jetzt auf der Warteliste. Wir benachrichtigen Dich, wenn Plätze frei werden.";
            BookingResponse::success(message, create_event_counters(&mut client).await?)
        }
        BookingResult::BookedOut => {
            error!(
                "Booking failed because Event ({}) was overbooked.",
                booking.event_id
            );
            BookingResponse::failure(MESSAGE_FAIL)
        }
    };

    Ok(result)
}

async fn subscribe_to_updates(
    client: &mut FirestoreClient<InterceptedService<Channel, GouthInterceptor>>,
    booking: &EventBooking,
    event: &Event,
) -> anyhow::Result<()> {
    // only subscribe to updates if updates field is true
    if booking.updates.unwrap_or(false) == false {
        return Ok(());
    }

    // TODO: subscribe to updates

    Ok(())
}
