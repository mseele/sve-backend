use crate::models::{BookingResponse, EventBooking, EventCounter, Subscription};
use crate::models::{Event, PartialEvent};
use crate::sheets::{self, BookingDetection};
use crate::store::{self, BookingResult, GouthInterceptor};
use anyhow::{bail, Context, Result};
use googapis::google::firestore::v1::firestore_client::FirestoreClient;
use log::{error, info, warn};
use std::str::from_utf8;
use tonic::codegen::InterceptedService;
use tonic::transport::Channel;

const MESSAGE_FAIL: &str =
    "Leider ist etwas schief gelaufen. Bitte versuche es später noch einmal.";

pub async fn get_events(all: Option<bool>, beta: Option<bool>) -> Result<Vec<Event>> {
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

pub async fn get_event_counters() -> Result<Vec<EventCounter>> {
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

pub async fn prebooking(hash: String) -> BookingResponse {
    match do_prebooking(hash).await {
        Ok(response) => response,
        Err(e) => {
            error!("Prebooking failed: {}", e);
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
) -> Result<Vec<EventCounter>> {
    let event_counters = get_and_filter_events(client, None, None)
        .await?
        .into_iter()
        .map(|event| event.into())
        .collect::<Vec<EventCounter>>();

    Ok(event_counters)
}

async fn do_booking(booking: EventBooking) -> Result<BookingResponse> {
    let mut client = store::get_client().await?;
    Ok(book_event(&mut client, booking).await?)
}

async fn book_event(
    client: &mut FirestoreClient<InterceptedService<Channel, GouthInterceptor>>,
    booking: EventBooking,
) -> Result<BookingResponse> {
    let booking_result = &store::book_event(client, &booking.event_id).await?;
    let result = match booking_result {
        BookingResult::Booked(event) | BookingResult::WaitingList(event) => {
            sheets::save_booking(&booking, &event).await?;
            subscribe_to_updates(client, &booking, &event).await?;
            send_mail(&booking, &event, &booking_result).await?;
            info!("Booking of Event {} was successfull", booking.event_id);
            let message;
            if let BookingResult::Booked(_) = booking_result {
                message = "Die Buchung war erfolgreich. Du bekommst in den nächsten Minuten eine Bestätigung per E-Mail.";
            } else {
                message = "Du stehst jetzt auf der Warteliste. Wir benachrichtigen Dich, wenn Plätze frei werden.";
            }
            BookingResponse::success(message, create_event_counters(client).await?)
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

async fn do_prebooking(hash: String) -> Result<BookingResponse> {
    let bytes = base64::decode(&hash)
        .with_context(|| format!("Error decoding the prebooking hash {}", &hash))?;
    let decoded = from_utf8(&bytes).with_context(|| {
        format!(
            "Error converting the decoded prebooking hash {} into a string slice",
            &hash
        )
    })?;
    let splitted = decoded.split('#').collect::<Vec<_>>();

    // fail if the length is not correct
    if splitted.len() != 8 {
        bail!(
            "Booking failed beacuse spitted prebooking hash ({}) has an invalid length: {}",
            decoded,
            splitted.len()
        );
    }

    let booking = EventBooking::new(
        splitted[0].into(),
        splitted[1].into(),
        splitted[2].into(),
        splitted[3].into(),
        splitted[4].into(),
        splitted[5].into(),
        Some(splitted[6].into()),
        Some("J".eq(splitted[7])),
        Some(false),
        Some(String::from("Pre-Booking")),
    );

    let mut client = store::get_client().await?;
    let event = store::get_event(&mut client, &booking.event_id).await?;

    // prebooking is only available for beta events
    if !event.beta {
        warn!("Prebooking has ended for booking {:?}", booking);
        return Ok(BookingResponse::failure(
            "Der Buchungslink ist nicht mehr gültig da die Frühbuchungsphase zu Ende ist.",
        ));
    }

    // check if prebooking has been used already
    let result = sheets::detect_booking(&booking, &event).await?;
    if let BookingDetection::Booked = result {
        warn!(
            "Prebooking link data has been detected and invalidated for booking {:?}",
            booking
        );
        return Ok(BookingResponse::failure(
            "Der Buchungslink wurde schon benutzt und ist daher ungültig.",
        ));
    }

    Ok(book_event(&mut client, booking).await?)
}

async fn subscribe_to_updates(
    client: &mut FirestoreClient<InterceptedService<Channel, GouthInterceptor>>,
    booking: &EventBooking,
    event: &Event,
) -> Result<()> {
    // only subscribe to updates if updates field is true
    if booking.updates.unwrap_or(false) == false {
        return Ok(());
    }
    // TODO: try to get rid of clone
    let subscription =
        Subscription::new(booking.email.clone(), vec![event.event_type.clone().into()]);
    super::news::subscribe_to_news(client, subscription, false).await?;

    Ok(())
}

async fn send_mail(
    booking: &EventBooking,
    event: &Event,
    booking_result: &BookingResult,
) -> Result<()> {
    // FIXME: create email

    Ok(())
}
