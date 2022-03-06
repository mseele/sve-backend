use crate::email;
use crate::models::{BookingResponse, EventBooking, EventCounter, EventType, Subscription};
use crate::models::{Event, PartialEvent};
use crate::sheets::{self, BookingDetection};
use crate::store::{self, BookingResult, GouthInterceptor};
use anyhow::{bail, Context, Result};
use chrono::{prelude::*, Duration};
use googapis::google::firestore::v1::firestore_client::FirestoreClient;
use log::{error, info, warn};
use regex::Regex;
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
    let email_account = match &event.alt_email_address {
        Some(email_address) => email::get_account_by_address(email_address),
        None => email::get_account_by_type(event.event_type.into()),
    }?;
    let subject_prefix = format!(
        "[{}@SVE]",
        match event.event_type {
            EventType::Fitness => "Fitness",
            EventType::Events => "Events",
        }
    );
    let subject;
    let template;
    if let BookingResult::Booked(_) = booking_result {
        subject = format!("{} Bestätigung Buchung", subject_prefix);
        template = &event.booking_template;
    } else {
        subject = format!("{} Bestätigung Warteliste", subject_prefix);
        template = &event.waiting_template;
    }

    let message = email_account
        .new_message()?
        .to(booking.email.parse()?)
        .bcc(email_account.mailbox()?)
        .subject(subject)
        .body(create_body(template, booking, event))?;

    email::send_message(&email_account, message).await?;

    Ok(())
}

fn create_body(template: &str, booking: &EventBooking, event: &Event) -> String {
    let mut body = template
        .replace("${firstname}", booking.first_name.trim())
        .replace("${lastname}", booking.last_name.trim())
        .replace("${name}", event.name.trim())
        .replace("${location}", &event.location)
        .replace("${price}", &booking.cost_as_string(event))
        .replace("${dates}", &format_dates(&event));
    body = replace_payday(body, &event);
    if booking.updates.unwrap_or(false) {
        body.push_str(
            format!(
                "

PS: Ab sofort erhältst Du automatisch eine E-Mail, sobald neue {} online sind.
{}",
                match event.event_type {
                    EventType::Fitness => "Kursangebote",
                    EventType::Events => "Events",
                },
                super::news::UNSUBSCRIBE_MESSAGE
            )
            .as_str(),
        )
    }
    body
}

fn format_dates(event: &Event) -> String {
    event
        .dates
        .iter()
        .map(|d| {
            DateTime::<Utc>::from_utc(*d, Utc)
                .format_localized("- %a, %d. %B %Y, %H:%M Uhr", Locale::de_DE)
                .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn replace_payday(body: String, event: &Event) -> String {
    if let Some(first_date) = event.dates.first() {
        let mut payday_replace_str = "${payday}";
        let mut days = 14;
        let payday_regex = Regex::new(r"\$\{payday:(\d+)\}").expect("regex is valid");
        if let Some(find_result) = payday_regex.find(&body) {
            payday_replace_str = &body[find_result.start()..find_result.end()];
            if let Some(captures) = payday_regex.captures(&body) {
                days = captures
                    .get(1)
                    .map_or("", |m| m.as_str())
                    .parse::<i32>()
                    .expect("group is an integer");
            }
        }
        let mut payday = *first_date - Duration::days(days.into());
        let tomorrow = Utc::now().naive_utc() + Duration::days(1);
        if payday < tomorrow {
            payday = tomorrow
        }

        return body.replace(
            payday_replace_str,
            &DateTime::<Utc>::from_utc(payday, Utc)
                .format_localized("%d. %B", Locale::de_DE)
                .to_string(),
        );
    }

    body
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_replace_payday() {
        // event starts in 3 weeks
        let event = new_event(vec![Utc::now().naive_utc() + Duration::weeks(3)]);
        assert_eq!(
            replace_payday("${payday}".into(), &event),
            format_payday(Utc::now() + Duration::weeks(1))
        );
        assert_eq!(
            replace_payday("${payday:7}".into(), &event),
            format_payday(Utc::now() + Duration::weeks(2))
        );
        assert_eq!(
            replace_payday("${payday:0}".into(), &event),
            format_payday(Utc::now() + Duration::weeks(3))
        );
        let tomorrow = (Utc::now() + Duration::days(1))
            .format_localized("%d. %B", Locale::de_DE)
            .to_string();
        assert_eq!(replace_payday("${payday:21}".into(), &event), tomorrow);
        assert_eq!(replace_payday("${payday:28}".into(), &event), tomorrow);

        // event starts in 3 days
        let event = new_event(vec![Utc::now().naive_utc() + Duration::days(3)]);
        assert_eq!(
            replace_payday("${payday:1}".into(), &event),
            format_payday(Utc::now() + Duration::days(2))
        );
        assert_eq!(replace_payday("${payday:2}".into(), &event), tomorrow);
        assert_eq!(replace_payday("${payday:3}".into(), &event), tomorrow);
        assert_eq!(replace_payday("${payday:14}".into(), &event), tomorrow);

        // event starts today
        let event = new_event(vec![Utc::now().naive_utc()]);
        assert_eq!(replace_payday("${payday}".into(), &event), tomorrow);
        assert_eq!(replace_payday("${payday:7}".into(), &event), tomorrow);

        // event started yesterday
        let event = new_event(vec![Utc::now().naive_utc() - Duration::days(1)]);
        assert_eq!(replace_payday("${payday}".into(), &event), tomorrow);
        assert_eq!(replace_payday("${payday:7}".into(), &event), tomorrow);
    }

    fn format_payday(date_time: DateTime<Utc>) -> String {
        date_time
            .format_localized("%d. %B", Locale::de_DE)
            .to_string()
    }

    fn new_event(dates: Vec<NaiveDateTime>) -> Event {
        Event::new(
            String::from("id"),
            String::from("sheet_id"),
            0,
            EventType::Fitness,
            String::from("name"),
            0,
            true,
            false,
            String::from("short_description"),
            String::from("description"),
            String::from("image"),
            true,
            dates,
            None,
            0,
            0,
            0,
            0.0,
            0.0,
            0,
            0,
            String::from("location"),
            String::from("booking_template"),
            String::from("waiting_template"),
            None,
            None,
            false,
        )
    }
}
