use super::csv::PaymentRecord;
use super::template;
use crate::db::BookingResult;
use crate::email;
use crate::models::{
    BookingResponse, Email, EmailAttachment, Event, EventBooking, EventCounter, EventEmail,
    EventId, EventType, LifecycleStatus, MessageType, NewsSubscription, PartialEvent, ToEuro,
    UnpaidEventBooking, VerifyPaymentBookingRecord, VerifyPaymentResult,
};
use crate::{db, hashids};
use anyhow::{anyhow, bail, Context, Result};
use chrono::{Date, DateTime, Duration, NaiveDate, Utc};
use encoding::Encoding;
use encoding::{all::ISO_8859_1, DecoderTrap};
use lazy_static::lazy_static;
use lettre::message::SinglePart;
use log::{error, info, warn};
use regex::Regex;
use sqlx::PgPool;
use std::collections::{BTreeMap, HashMap, HashSet};

const MESSAGE_FAIL: &str =
    "Leider ist etwas schief gelaufen. Bitte versuche es später noch einmal.";

pub(crate) async fn get_events(
    pool: &PgPool,
    beta: Option<bool>,
    lifecycle_status: Option<Vec<LifecycleStatus>>,
    subscribers: Option<bool>,
) -> Result<Vec<Event>> {
    let lifecycle_status_list;
    if let Some(beta) = beta {
        lifecycle_status_list = Some(vec![into_lifecycle_status(beta)]);
    } else {
        lifecycle_status_list = lifecycle_status;
    }
    Ok(db::get_events(
        pool,
        true,
        lifecycle_status_list,
        subscribers.unwrap_or(false),
    )
    .await?)
}

pub(crate) async fn get_event_counters(pool: &PgPool, beta: bool) -> Result<Vec<EventCounter>> {
    Ok(db::get_event_counters(pool, into_lifecycle_status(beta)).await?)
}

pub(crate) async fn booking(pool: &PgPool, booking: EventBooking) -> BookingResponse {
    match book_event(pool, booking).await {
        Ok(response) => response,
        Err(e) => {
            error!("Booking failed: {:?}", e);
            BookingResponse::failure(MESSAGE_FAIL)
        }
    }
}

pub(crate) async fn prebooking(pool: &PgPool, hash: String) -> BookingResponse {
    match pre_book_event(pool, hash).await {
        Ok(response) => response,
        Err(e) => {
            error!("Prebooking failed: {:?}", e);
            BookingResponse::failure(MESSAGE_FAIL)
        }
    }
}

pub(crate) async fn update(pool: &PgPool, partial_event: PartialEvent) -> Result<Event> {
    let (event, event_schedule_change) = db::write_event(pool, partial_event).await?;
    if event_schedule_change
        && matches!(
            event.lifecycle_status,
            LifecycleStatus::Review | LifecycleStatus::Published | LifecycleStatus::Running
        )
    {
        let subject = format!("{} Terminänderung {}", event.subject_prefix(), event.name);
        let body = match event.event_type {
            EventType::Fitness => include_str!("../../templates/schedule_change_fitness.txt"),
            EventType::Events => include_str!("../../templates/schedule_change_events.txt"),
        };
        process_event_email(
            pool,
            event.clone(),
            Some(true),
            subject,
            body.into(),
            None,
            None,
        )
        .await?;
    }
    Ok(event)
}

pub(crate) async fn delete(pool: &PgPool, event_id: EventId) -> Result<()> {
    Ok(db::delete_event(pool, event_id).await?)
}

pub(crate) async fn verify_payments(
    pool: &PgPool,
    csv: String,
    csv_start_date: Option<NaiveDate>,
) -> Result<Vec<VerifyPaymentResult>> {
    let bytes = base64::decode(&csv)
        .with_context(|| format!("Error decoding the cvs content: {}", &csv))?;
    let csv = match ISO_8859_1.decode(&bytes, DecoderTrap::Strict) {
        Ok(value) => value,
        Err(e) => bail!("Decoding csv content with ISO 8859: {}", e.into_owned()),
    };

    let payment_records =
        actix_web::web::block(move || read_payment_records(&csv, csv_start_date)).await??;
    let payment_ids = payment_records
        .iter()
        .flat_map(|r| &r.payment_ids)
        .collect::<HashSet<_>>();
    let mut bookings = db::get_bookings_to_verify_payment(pool, payment_ids).await?;
    let (verified_payments, result) =
        compare_payment_records_with_bookings(&payment_records, &mut bookings)?;
    if verified_payments.len() > 0 {
        db::mark_as_payed(&pool, &verified_payments).await?;
    }

    Ok(result)
}

pub(crate) async fn get_unpaid_bookings(
    pool: &PgPool,
    event_type: EventType,
) -> Result<Vec<UnpaidEventBooking>> {
    let result = db::get_event_bookings_without_payment(&pool, event_type).await?;

    let mut bookings = Vec::new();

    for (mut booking, booking_insert_date, first_event_date, event_template) in result.into_iter() {
        if let Some(first_event_date) = first_event_date {
            let booking_date;
            if let Some(payment_reminder_sent) = booking.payment_reminder_sent {
                booking_date = payment_reminder_sent + Duration::days(3);
            } else {
                booking_date = booking_insert_date;
            }
            let due_in_days = calc_due_in_days(booking_date, first_event_date, event_template)?;
            booking.due_in_days = Some(due_in_days);
        }
        bookings.push(booking);
    }

    Ok(bookings)
}

/// calculate the days until the booking should be payed
fn calc_due_in_days(
    booking_date: DateTime<Utc>,
    first_event_date: DateTime<Utc>,
    event_template: String,
) -> Result<i64> {
    // initialize regex
    lazy_static! {
        static ref PAYDAY_REGEX: Regex =
            Regex::new(r"\{\{payday\s+(\d+)\}\}").expect("regex is valid");
    }

    // calculate custom days - if defined in the event template
    let custom_days;
    if let Some(captures) = PAYDAY_REGEX.captures(&event_template) {
        custom_days = Some(
            captures
                .get(1)
                .map_or("", |m| m.as_str())
                .parse::<i64>()
                .expect("custom day is an integer"),
        );
    } else {
        custom_days = None;
    }

    // get the payday for the event
    let payday = calculate_payday(&booking_date, &first_event_date, custom_days);

    // calculate due in days
    let due_in_days = payday - Utc::today();

    Ok(due_in_days.num_days())
}

pub(crate) async fn update_payment(
    pool: &PgPool,
    booking_id: i32,
    update_payment: bool,
) -> Result<()> {
    Ok(db::update_payment(&pool, booking_id, update_payment).await?)
}

pub(crate) async fn cancel_booking(pool: &PgPool, booking_id: i32) -> Result<()> {
    let (event, canceled_booking, waiting_list_booking) =
        db::cancel_event_booking(&pool, booking_id).await?;

    let email_account = email::get_account_by_type(event.event_type.into())?;
    let mut messages = Vec::new();

    // create cancellation confirmation email
    let subject = format!("{} Stornierung Buchung", event.subject_prefix());
    let body = match event.event_type {
        EventType::Fitness => include_str!("../../templates/cancel_booking_fitness.txt"),
        EventType::Events => include_str!("../../templates/cancel_booking_events.txt"),
    };
    let body = template::render_booking(&body, &canceled_booking, &event, None, None, None)?;
    messages.push(
        email_account
            .new_message()?
            .to(canceled_booking.email.parse()?)
            .bcc(email_account.mailbox()?)
            .subject(subject)
            .singlepart(SinglePart::plain(body))?,
    );

    // create booking confirmation email for the new booking
    if let Some((new_booking, payment_id)) = waiting_list_booking {
        let subject = format!("{} Bestätigung Buchung", event.subject_prefix());
        let body = template::render_booking(
            &event.booking_template,
            &new_booking,
            &event,
            Some(payment_id),
            None,
            Some(false),
        )?;

        messages.push(
            email_account
                .new_message()?
                .to(new_booking.email.parse()?)
                .bcc(email_account.mailbox()?)
                .subject(subject)
                .singlepart(SinglePart::plain(body))?,
        );
    }

    email::send_messages(&email_account, messages).await?;

    Ok(())
}

pub(crate) async fn send_event_email(pool: &PgPool, data: EventEmail) -> Result<()> {
    if !data.bookings && !data.waiting_list {
        bail!("Either bookings or waiting list option need to be selected to send an event email.")
    }
    let enrolled = if data.bookings && !data.waiting_list {
        Some(true)
    } else if data.waiting_list && !data.bookings {
        Some(false)
    } else {
        None
    };

    let event = db::get_event(pool, &data.event_id)
        .await?
        .ok_or_else(|| anyhow!("Found no event with id '{}'", data.event_id))?;

    process_event_email(
        pool,
        event,
        enrolled,
        data.subject,
        data.body,
        data.attachments,
        data.prebooking_event_id,
    )
    .await?;

    Ok(())
}

/// send a reminder email for each events that starts next week
pub(crate) async fn send_event_reminders(pool: &PgPool) -> Result<usize> {
    // get all events where a event reminder should be send to the subscribers
    let events = db::get_reminder_events(pool).await?;

    // process each event
    for event in &events {
        // prepare for message generation
        let email_account = email::get_account_by_type(event.event_type.into())?;
        let message_type: MessageType = event.event_type.into();
        let mut messages = Vec::new();

        // get the subject and body (depending on the event type)
        let (subject, body) = match event.event_type {
            EventType::Fitness => (
                format!("{} Info zum Kursstart", event.subject_prefix()),
                include_str!("../../templates/event_reminder_fitness.txt"),
            ),
            EventType::Events => (
                format!("{} Info zum Eventstart", event.subject_prefix()),
                include_str!("../../templates/event_reminder_events.txt"),
            ),
        };

        // iterate all event subscribers (Option should never be None)
        if let Some(subscribers) = &event.subscribers {
            for subscriber in subscribers {
                // render the body for the email...
                let body = template::render_event_reminder(&body, &event, subscriber)?;

                // ...and push the email into the messages list
                messages.push(
                    Email::new(
                        message_type,
                        subscriber.email.clone(),
                        subject.clone(),
                        body,
                        None,
                    )
                    .into_message(&email_account)?,
                );
            }

            // send reminder emails to all event subribers
            email::send_messages(&email_account, messages).await?;

            // mark reminder has been sent to the event
            // (to avoid duplicate sending of reminder emails)
            db::mark_as_reminder_sent(pool, &event.id).await?;
        }
    }

    // return the count of events for which the reminder has been send
    Ok(events.len())
}

/// send a reminder email for all bookings which are due with payment
pub(crate) async fn send_payment_reminders(pool: &PgPool, event_type: EventType) -> Result<usize> {
    // get unpaid bookings and filter for bookings that are due with payment
    let bookings = get_unpaid_bookings(&pool, event_type)
        .await?
        .into_iter()
        .filter(|booking| match booking.due_in_days {
            Some(due_in_days) if due_in_days < 0 => true,
            _ => false,
        })
        .collect::<Vec<_>>();

    // prepare for message generation
    let email_account = email::get_account_by_type(event_type.into())?;
    let message_type: MessageType = event_type.into();
    let mut messages = Vec::new();

    // get the subject and body (depending on the event type)
    let subject = format!("{} Zahlungserinnerung", event_type.subject_prefix());
    let body = match event_type {
        EventType::Fitness => include_str!("../../templates/payment_reminder_fitness.txt"),
        EventType::Events => include_str!("../../templates/payment_reminder_events.txt"),
    };

    let mut event_cache = HashMap::new();
    for booking in bookings.iter() {
        // get the event from the cache of from the database
        let key = booking.event_id;
        if !event_cache.contains_key(&key) {
            let value = db::get_event(&pool, &booking.event_id)
                .await?
                .ok_or_else(|| anyhow!("Event with id '{}' is missing", key))?;
            event_cache.insert(key, value);
        }
        let event = event_cache
            .get(&key)
            .ok_or_else(|| anyhow!("Event with id '{}' is not in the cache", key))?;

        // render the body for the email...
        let body = template::render_payment_reminder(&body, &event, &booking)?;

        // ...and push the email into the messages list
        messages.push(
            Email::new(
                message_type,
                booking.email.clone(),
                subject.clone(),
                body,
                None,
            )
            .into_message(&email_account)?,
        );
    }

    // send reminder emails to all bookings due with payment
    email::send_messages(&email_account, messages).await?;

    // mark payment reminder has been sent to the bookings
    // (to avoid duplicate sending of reminder emails)
    let booking_ids = bookings
        .into_iter()
        .map(|booking| booking.booking_id)
        .collect::<Vec<_>>();
    db::mark_as_payment_reminder_sent(pool, &booking_ids).await?;

    Ok(booking_ids.len())
}

async fn process_event_email(
    pool: &PgPool,
    event: Event,
    enrolled: Option<bool>,
    subject: String,
    body: String,
    attachments: Option<Vec<EmailAttachment>>,
    prebooking_event_id: Option<EventId>,
) -> Result<()> {
    let bookings = db::get_bookings(pool, &event.id, enrolled).await?;
    if bookings.is_empty() {
        return Ok(());
    }

    let email_account = email::get_account_by_type(event.event_type.into())?;
    let message_type: MessageType = event.event_type.into();
    let mut messages = Vec::new();

    for (booking, subscriber_id, payment_id) in bookings {
        let prebooking_link;
        if let Some(event_id) = prebooking_event_id {
            prebooking_link = Some(create_prebooking_link(
                event.event_type,
                event_id,
                subscriber_id,
            )?);
        } else {
            prebooking_link = None;
        }

        let body = template::render_booking(
            &body,
            &booking,
            &event,
            Some(payment_id),
            prebooking_link,
            None,
        )?;

        let attachments = match &attachments {
            Some(attachments) => Some(
                attachments
                    .into_iter()
                    .map(|attachment| attachment.clone())
                    .collect::<Vec<_>>(),
            ),
            None => None,
        };

        messages.push(
            Email::new(
                message_type,
                booking.email,
                subject.clone(),
                body,
                attachments,
            )
            .into_message(&email_account)?,
        );
    }

    email::send_messages(&email_account, messages).await?;

    Ok(())
}

fn into_lifecycle_status(beta: bool) -> LifecycleStatus {
    if beta {
        LifecycleStatus::Review
    } else {
        LifecycleStatus::Published
    }
}

async fn book_event(pool: &PgPool, booking: EventBooking) -> Result<BookingResponse> {
    let booking_result = db::book_event(pool, &booking).await?;
    let booking_response = match booking_result {
        BookingResult::Booked(event, counter, payment_id) => {
            process_booking(pool, &booking, event, counter, true, payment_id).await?
        }
        BookingResult::WaitingList(event, counter, payment_id) => {
            process_booking(pool, &booking, event, counter, false, payment_id).await?
        }
        BookingResult::DuplicateBooking => {
            error!(
                "Event ({}) booking failed because a duplicate booking has been detected.",
                booking.event_id
            );
            BookingResponse::failure("Wir haben schon eine Buchung mit diesen Anmeldedaten erkannt. Bitte verwende für weitere Buchungen andere Anmeldedaten.")
        }
        BookingResult::NotBookable => {
            error!(
                "Event ({}) booking failed because the event is in a unbookable state.",
                booking.event_id
            );
            BookingResponse::failure("Das Event kann aktuell leider nicht gebucht werden.")
        }
        BookingResult::BookedOut => {
            error!(
                "Booking failed because Event ({}) was overbooked.",
                booking.event_id
            );
            BookingResponse::failure(MESSAGE_FAIL)
        }
    };

    Ok(booking_response)
}

async fn pre_book_event(pool: &PgPool, hash: String) -> Result<BookingResponse> {
    let ids = hashids::decode(&hash)
        .with_context(|| format!("Error decoding the prebooking hash {} into ids", &hash))?;

    // fail if the length is not correct
    if ids.len() != 2 {
        bail!(
            "Booking failed because decoded prebooking hash ({}) has an invalid length: {}",
            hash,
            ids.len()
        );
    }

    let event_id = EventId::from(i32::try_from(ids[0])?);
    let subscriber_id = ids[1].try_into()?;

    let (booking_result, booking) = db::pre_book_event(pool, event_id, subscriber_id).await?;
    let booking_response = match booking_result {
        BookingResult::Booked(event, counter, payment_id) => {
            process_booking(
                pool,
                &booking
                    .ok_or_else(|| anyhow!("Found no 'virtual' booking. Never should be here."))?,
                event,
                counter,
                true,
                payment_id,
            )
            .await?
        }
        BookingResult::WaitingList(event, counter, payment_id) => {
            process_booking(
                pool,
                &booking
                    .ok_or_else(|| anyhow!("Found no 'virtual' booking. Never should be here."))?,
                event,
                counter,
                false,
                payment_id,
            )
            .await?
        }
        BookingResult::DuplicateBooking => {
            warn!(
                "Prebooking link data has been detected and invalidated for booking {:?}",
                booking
            );
            BookingResponse::failure("Der Buchungslink wurde schon benutzt und ist daher ungültig.")
        }
        BookingResult::NotBookable => {
            warn!(
                "Prebooking is not possible because the event is in a unbookable state for booking {:?}",
                booking
            );
            BookingResponse::failure("Das Event kann aktuell leider nicht gebucht werden.")
        }
        BookingResult::BookedOut => {
            BookingResponse::failure("Das Event ist leider schon ausgebucht.")
        }
    };

    Ok(booking_response)
}

async fn process_booking(
    pool: &PgPool,
    booking: &EventBooking,
    event: Event,
    counter: Vec<EventCounter>,
    booked: bool,
    payment_id: String,
) -> Result<BookingResponse> {
    subscribe_to_updates(pool, booking, &event).await?;
    send_booking_mail(booking, &event, booked, payment_id).await?;
    info!("Booking of Event {} was successfull", booking.event_id);
    let message;
    if booked {
        message = "Die Buchung war erfolgreich. Du bekommst in den nächsten Minuten eine Bestätigung per E-Mail.";
    } else {
        message = "Du stehst jetzt auf der Warteliste. Wir benachrichtigen Dich, wenn Plätze frei werden.";
    }
    Ok(BookingResponse::success(message, counter))
}

async fn subscribe_to_updates(pool: &PgPool, booking: &EventBooking, event: &Event) -> Result<()> {
    // only subscribe to updates if updates field is true
    if booking.updates.unwrap_or(false) == false {
        return Ok(());
    }
    let subscription =
        NewsSubscription::new(booking.email.clone(), vec![event.event_type.clone().into()]);
    super::news::subscribe_to_news(pool, subscription, false).await?;

    Ok(())
}

async fn send_booking_mail(
    booking: &EventBooking,
    event: &Event,
    booked: bool,
    payment_id: String,
) -> Result<()> {
    let email_account = match &event.alt_email_address {
        Some(email_address) => email::get_account_by_address(email_address),
        None => email::get_account_by_type(event.event_type.into()),
    }?;
    let subject;
    let template;
    let opt_payment_id;
    if booked {
        subject = format!("{} Bestätigung Buchung", event.subject_prefix());
        template = &event.booking_template;
        opt_payment_id = Some(payment_id);
    } else {
        subject = format!("{} Bestätigung Warteliste", event.subject_prefix());
        template = &event.waiting_template;
        opt_payment_id = None;
    }

    let mut body = template::render_booking(template, booking, event, opt_payment_id, None, None)?;

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

    let message = email_account
        .new_message()?
        .to(booking.email.parse()?)
        .bcc(email_account.mailbox()?)
        .subject(subject)
        .singlepart(SinglePart::plain(body))?;

    email::send_message(&email_account, message).await?;

    Ok(())
}

fn create_prebooking_link(
    event_type: EventType,
    event_id: EventId,
    subscriber_id: i32,
) -> Result<String> {
    let mut url = String::from("https://www.sv-eutingen.de/");
    url.push_str(match event_type {
        EventType::Fitness => "fitness",
        EventType::Events => "events",
    });
    url.push_str("/buchung?code=");

    // create the code
    url.push_str(&hashids::encode(&[
        event_id.into_inner().try_into()?,
        subscriber_id.try_into()?,
    ]));

    return Ok(url);
}

fn read_payment_records(
    csv: &str,
    csv_start_date: Option<NaiveDate>,
) -> Result<Vec<PaymentRecord>> {
    let mut records = Vec::new();

    for record in super::csv::read(csv)? {
        // skip all records that are older than the start date
        if let Some(start_date) = csv_start_date {
            if record.date < start_date {
                continue;
            }
        }
        records.push(record);
    }

    Ok(records)
}

fn compare_payment_records_with_bookings(
    payment_records: &Vec<PaymentRecord>,
    bookings: &mut Vec<VerifyPaymentBookingRecord>,
) -> Result<(HashMap<i32, String>, Vec<VerifyPaymentResult>)> {
    let mut verified_payment_bookings = Vec::new();
    let mut verified_ibans = HashMap::new();
    let mut payment_bookings_with_errors = BTreeMap::new();
    let mut non_matching_payment_records = Vec::new();

    let mut bookings = bookings
        .iter()
        .map(|booking| (&booking.payment_id, booking))
        .collect::<HashMap<_, _>>();

    for payment_record in payment_records {
        // TODO: add support for payment record with multiple payment ids

        if payment_record.payment_ids.len() < 1 || payment_record.payment_ids.len() > 1 {
            non_matching_payment_records.push(format!(
                "{} / {} / {} / {}",
                payment_record.payee,
                payment_record.payee_iban,
                payment_record.purpose,
                payment_record.volumne.to_euro()
            ));
            continue;
        }
        let payment_id = payment_record.payment_ids.iter().next().ok_or_else(|| {
            anyhow!("Payment record is missing payment ids. Never should be here.")
        })?;

        let booking = bookings.remove(&payment_id);
        if let Some(booking) = booking {
            if booking.payed.is_some() {
                payment_bookings_with_errors
                    .entry(payment_id)
                    .or_insert_with(|| Vec::new())
                    .push(format!(
                        "Doppelt bezahlt: Buchung ist schon als bezahlt markiert"
                    ));
            }

            if booking.enrolled && booking.canceled.is_some() {
                payment_bookings_with_errors
                    .entry(payment_id)
                    .or_insert_with(|| Vec::new())
                    .push(format!(
                        "Falsch bezahlt: Buchung ist als storniert markiert"
                    ));
            }

            if !booking.enrolled {
                payment_bookings_with_errors
                    .entry(payment_id)
                    .or_insert_with(|| Vec::new())
                    .push(format!(
                        "Falsch bezahlt: Buchung ist von auf der Warteliste"
                    ));
            }

            let record_volumne = payment_record.volumne.to_euro();
            let booking_cost = booking.cost.to_euro();
            if !record_volumne.eq(&booking_cost) {
                payment_bookings_with_errors
                    .entry(payment_id)
                    .or_insert_with(|| Vec::new())
                    .push(format!(
                        "Betrag falsch: erwartet {booking_cost} != überwiesen {record_volumne}"
                    ));
            }

            if !payment_bookings_with_errors.contains_key(&booking.payment_id) {
                verified_payment_bookings.push(booking);
                verified_ibans.insert(booking.booking_id, payment_record.payee_iban.clone());
            }
        } else {
            non_matching_payment_records.push(format!(
                "{} / {} / {} / {}",
                payment_record.payee,
                payment_record.payee_iban,
                payment_record.purpose,
                payment_record.volumne.to_euro()
            ));
            break;
        }
    }
    verified_payment_bookings.sort_unstable_by(|a, b| a.payment_id.cmp(&b.payment_id));

    let mut compare_result = Vec::new();

    // first group are the successfull matches
    compare_result.push(VerifyPaymentResult::new(
        format!(
            "{} bezahlte {}",
            verified_payment_bookings.len(),
            match verified_payment_bookings.len() {
                1 => "Buchung",
                _ => "Buchungen",
            }
        ),
        verified_payment_bookings
            .iter()
            .map(|booking| booking.payment_id.clone())
            .collect::<Vec<_>>(),
    ));

    // second group are the matches with errors
    compare_result.push(VerifyPaymentResult::new(
        format!(
            "{} {} mit Problemen",
            payment_bookings_with_errors.len(),
            match payment_bookings_with_errors.len() {
                1 => "Buchung",
                _ => "Buchungen",
            }
        ),
        payment_bookings_with_errors
            .into_iter()
            .map(|(payment_id, mut errors)| {
                errors.insert(0, payment_id.clone());
                errors.join(" / ")
            })
            .collect::<Vec<_>>(),
    ));

    // third group are all non matching payment records
    compare_result.push(VerifyPaymentResult::new(
        format!(
            "{} nicht erkannte {}",
            non_matching_payment_records.len(),
            match non_matching_payment_records.len() {
                1 => "Buchung",
                _ => "Buchungen",
            }
        ),
        non_matching_payment_records,
    ));

    Ok((verified_ibans, compare_result))
}

/// Calculate the payday with the first event date.
pub(super) fn calculate_payday(
    booking_date: &DateTime<Utc>,
    first_event_date: &DateTime<Utc>,
    custom_days: Option<i64>,
) -> Date<Utc> {
    // default value is 14 days
    let mut days = 14;

    // overwrite with custom days - if available
    if let Some(custom_days) = custom_days {
        days = custom_days;
    }

    // calculated payday
    let mut payday = first_event_date.date() - Duration::days(days);

    // override the payday if it is before the day after the booking date
    let earliest_paypay = booking_date.date() + Duration::days(1);
    if payday < earliest_paypay {
        payday = earliest_paypay
    }

    payday
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;
    use bigdecimal::{BigDecimal, FromPrimitive};
    use chrono::{NaiveDate, Utc};
    use pretty_assertions::assert_eq;

    #[test]
    fn test_create_prebooking_link() {
        assert_eq!(
            create_prebooking_link(EventType::Fitness, 1.into(), 0.into()).unwrap(),
            format!(
                "https://www.sv-eutingen.de/fitness/buchung?code={}",
                hashids::encode(&[1, 0])
            )
        );
        assert_eq!(
            create_prebooking_link(EventType::Events, 2.into(), 1.into()).unwrap(),
            format!(
                "https://www.sv-eutingen.de/events/buchung?code={}",
                hashids::encode(&[2, 1])
            )
        );
    }

    #[test]
    fn test_compare_csv_with_bookings() {
        // matching bookings with and without errors
        let csv = r#";;;;;;;;;;;;;;;;
;;;;;;;;;;;;;;;;
Umsatzanzeige;;;;;;;;;;;;;;;;
;;;;;;;;;;;;;;;;
BLZ:;10517962;;Datum:;12.03.2022;;;;;;;;;;;;
Konto:;25862911;;Uhrzeit:;14:17:19;;;;;;;;;;;;
Abfrage von:;Paul Ehrlich;;Kontoinhaber:;Sportverein Eutingen im Gäu e.V;;;;;;;;;;;;
;;;;;;;;;;;;;;;;
Zeitraum:;;von:;01.03.2022;bis:;12.03.2022;;;;;;;;;;;
Betrag in Euro:;;von:;;bis:;;;;;;;;;;;;
Primanotanummer:;;von:;;bis:;;;;;;;;;;;;
Textschlüssel:;;von:;;bis:;;;;;;;;;;;;
;;;;;;;;;;;;;;;;
;;;;;;;;;;;;;;;;
;;;;;;;;;;;;;;;;
Buchungstag;Valuta;Textschlüssel;Primanota;Zahlungsempfänger;ZahlungsempfängerKto;ZahlungsempfängerIBAN;ZahlungsempfängerBLZ;ZahlungsempfängerBIC;Vorgang/Verwendungszweck;Kundenreferenz;Währung;Umsatz;Soll/Haben
09.03.2022;09.03.2022;16 Euro-Überweisung;801;Test GmbH;0;DE92500105174132432988;58629112;GENODES1VBH;Überweisung Rechnung Nr. 20219862 Kunde 106155 TAN: Auftrag nicht TAN-pflichtig, da Kleinbetragszahlung IBAN: DE92500105174132432988 BIC: GENODES1VBH;;EUR;24,15;S
09.03.2022;09.03.2022;51 Überweisungsgutschr.;931;Max Mustermann;0;DE62500105176261449571;10517962;SOLADES1FDS;22-1423;;EUR;27,00;H
10.03.2022;10.03.2022;54 Überweisungsgutschr.;931;Erika Mustermann;0;DE91500105176171781279;10517962;SOLADES1FDS;Erika 22-1425 Mustermann;;EUR;33,50;H
10.03.2022;10.03.2022;78 Euro-Überweisung;931;Lieschen Müller;0;DE21500105179625862911;10517962;GENODES1VBH;Lieschen Müller 22-1456;;EUR;27,00;H
10.03.2022;10.03.2022;90 Euro-Überweisung;931;Otto Normalverbraucher;0;DE21500105179625862911;10517962;GENODES1VBH;Otto Normalverbraucher, Test-Kurs,22-1467;;EUR;45,90;H
;;;;;;;;;;;;;
01.03.2022;;;;;;;;;;Anfangssaldo;EUR;10.000,00;H
09.03.2022;;;;;;;;;;Endsaldo;EUR;20.000,00;H
"#;
        let mut bookings = vec![
            VerifyPaymentBookingRecord::new(
                1,
                "Test-Kurs".into(),
                "Max Mustermann".into(),
                BigDecimal::from_i8(27).unwrap(),
                "22-1423".into(),
                None,
                true,
                None,
            ),
            VerifyPaymentBookingRecord::new(
                2,
                "Test-Kurs".into(),
                "Erika Mustermann".into(),
                BigDecimal::from_i8(27).unwrap(),
                "22-1425".into(),
                None,
                true,
                None,
            ),
            VerifyPaymentBookingRecord::new(
                3,
                "Test-Kurs".into(),
                "Lieschen Müller".into(),
                BigDecimal::from_i8(27).unwrap(),
                "22-1456".into(),
                None,
                true,
                Some(Utc::now()),
            ),
            VerifyPaymentBookingRecord::new(
                4,
                "Test-Kurs".into(),
                "Otto Normalverbraucher".into(),
                BigDecimal::from_str("45.90").unwrap(),
                "22-1467".into(),
                Some(Utc::now()),
                true,
                None,
            ),
        ];

        assert_eq!(
    compare_csv_with_bookings(csv, None, &mut bookings),
    (
        HashMap::from([(1, "DE62500105176261449571".into())]),
        vec![
            VerifyPaymentResult::new(
                "1 bezahlte Buchung".into(),
                vec!["22-1423".into()]
            ),
            VerifyPaymentResult::new(
                "3 Buchungen mit Problemen".into(),
                vec!["22-1425 / Betrag falsch: erwartet 27,00 € != überwiesen 33,50 €".into(),
                     "22-1456 / Doppelt bezahlt: Buchung ist schon als bezahlt markiert".into(),
                     "22-1467 / Falsch bezahlt: Buchung ist als storniert markiert".into()]
            ),
            VerifyPaymentResult::new(
                "1 nicht erkannte Buchung".into(),
                vec!["Test GmbH / DE92500105174132432988 / Überweisung Rechnung Nr. 20219862 Kunde 106155 TAN: Auftrag nicht TAN-pflichtig, da Kleinbetragszahlung IBAN: DE92500105174132432988 BIC: GENODES1VBH / -24,15 €".into()]
            )
        ]
    )
);

        // no matching bookings
        let csv = "Bezeichnung Auftragskonto;IBAN Auftragskonto;BIC Auftragskonto;Bankname Auftragskonto;Buchungstag;Valutadatum;Name Zahlungsbeteiligter;IBAN Zahlungsbeteiligter;BIC (SWIFT-Code) Zahlungsbeteiligter;Buchungstext;Verwendungszweck;Betrag;Waehrung;Saldo nach Buchung;Bemerkung;Kategorie;Steuerrelevant;Glaeubiger ID;Mandatsreferenz
Festgeldkonto (Tagesgeld);DE68500105173456568557;GENODES1FDS;VOLKSBANK IM KREIS FREUDENSTADT;09.03.2022;09.03.2022;Test GmbH;DE92500105174132432988;GENODES1VBH;16 Euro-Überweisung;Überweisung Rechnung Nr. 20219862 Kunde 106155 TAN: Auftrag nicht TAN-pflichtig, da Kleinbetragszahlung IBAN: DE92500105174132432988 BIC: GENODES1VBH;-24,15;EUR;260,00;;;;;
";

        let mut bookings = vec![
            VerifyPaymentBookingRecord::new(
                1,
                "Test-Kurs".into(),
                "Max Mustermann".into(),
                BigDecimal::from_i8(27).unwrap(),
                "22-1423".into(),
                None,
                true,
                None,
            ),
            VerifyPaymentBookingRecord::new(
                2,
                "Test-Kurs".into(),
                "Erika Mustermann".into(),
                BigDecimal::from_i8(27).unwrap(),
                "22-1425".into(),
                None,
                true,
                None,
            ),
            VerifyPaymentBookingRecord::new(
                3,
                "Test-Kurs".into(),
                "Lieschen Müller".into(),
                BigDecimal::from_i8(27).unwrap(),
                "22-1456".into(),
                None,
                true,
                Some(Utc::now()),
            ),
            VerifyPaymentBookingRecord::new(
                4,
                "Test-Kurs".into(),
                "Otto Normalverbraucher".into(),
                BigDecimal::from_str("45.90").unwrap(),
                "22-1467".into(),
                None,
                true,
                None,
            ),
        ];

        assert_eq!(
    compare_csv_with_bookings(csv, None, &mut bookings),
    (
        HashMap::new(),
        vec![
            VerifyPaymentResult::new("0 bezahlte Buchungen".into(), vec![]),
            VerifyPaymentResult::new("0 Buchungen mit Problemen".into(), vec![]),
            VerifyPaymentResult::new(
                "1 nicht erkannte Buchung".into(),
                vec!["Test GmbH / DE92500105174132432988 / Überweisung Rechnung Nr. 20219862 Kunde 106155 TAN: Auftrag nicht TAN-pflichtig, da Kleinbetragszahlung IBAN: DE92500105174132432988 BIC: GENODES1VBH / -24,15 €".into()]
            )
        ]
    )
);

        // matching bookings with and without errors
        let csv = ";;;;;;;;;;;;;;;;
;;;;;;;;;;;;;;;;
Umsatzanzeige;;;;;;;;;;;;;;;;
;;;;;;;;;;;;;;;;
BLZ:;10517962;;Datum:;12.03.2022;;;;;;;;;;;;
Konto:;25862911;;Uhrzeit:;14:17:19;;;;;;;;;;;;
Abfrage von:;Paul Ehrlich;;Kontoinhaber:;Sportverein Eutingen im Gäu e.V;;;;;;;;;;;;
;;;;;;;;;;;;;;;;
Zeitraum:;;von:;01.03.2022;bis:;12.03.2022;;;;;;;;;;;
Betrag in Euro:;;von:;;bis:;;;;;;;;;;;;
Primanotanummer:;;von:;;bis:;;;;;;;;;;;;
Textschlüssel:;;von:;;bis:;;;;;;;;;;;;
;;;;;;;;;;;;;;;;
;;;;;;;;;;;;;;;;
;;;;;;;;;;;;;;;;
Buchungstag;Valuta;Textschlüssel;Primanota;Zahlungsempfänger;ZahlungsempfängerKto;ZahlungsempfängerIBAN;ZahlungsempfängerBLZ;ZahlungsempfängerBIC;Vorgang/Verwendungszweck;Kundenreferenz;Währung;Umsatz;Soll/Haben
09.03.2022;09.03.2022;16 Euro-Überweisung;801;Test GmbH;0;DE92500105174132432988;58629112;GENODES1VBH;Überweisung Rechnung Nr. 20219862 Kunde 106155 TAN: Auftrag nicht TAN-pflichtig, da Kleinbetragszahlung IBAN: DE92500105174132432988 BIC: GENODES1VBH;;EUR;24,15;S
09.03.2022;09.03.2022;51 Überweisungsgutschr.;931;Max Mustermann;0;DE62500105176261449571;10517962;SOLADES1FDS;22-1423;;EUR;27,00;H
10.03.2022;10.03.2022;54 Überweisungsgutschr.;931;Erika Mustermann;0;DE91500105176171781279;10517962;SOLADES1FDS;Erika 22-1425 Mustermann;;EUR;33,50;H
10.03.2022;10.03.2022;78 Euro-Überweisung;931;Lieschen Müller;0;DE21500105179625862911;10517962;GENODES1VBH;Lieschen Müller 22-1456;;EUR;27,00;H
11.03.2022;11.03.2022;90 Euro-Überweisung;931;Otto Normalverbraucher;0;DE21500105179625862911;10517962;GENODES1VBH;Otto Normalverbraucher, Test-Kurs,22-1467;;EUR;45,90;H
;;;;;;;;;;;;;
01.03.2022;;;;;;;;;;Anfangssaldo;EUR;10.000,00;H
09.03.2022;;;;;;;;;;Endsaldo;EUR;20.000,00;H
";
        let mut bookings = vec![
            VerifyPaymentBookingRecord::new(
                1,
                "Test-Kurs".into(),
                "Max Mustermann".into(),
                BigDecimal::from_i8(27).unwrap(),
                "22-1423".into(),
                None,
                true,
                None,
            ),
            VerifyPaymentBookingRecord::new(
                2,
                "Test-Kurs".into(),
                "Erika Mustermann".into(),
                BigDecimal::from_i8(27).unwrap(),
                "22-1425".into(),
                None,
                true,
                None,
            ),
            VerifyPaymentBookingRecord::new(
                3,
                "Test-Kurs".into(),
                "Lieschen Müller".into(),
                BigDecimal::from_i8(27).unwrap(),
                "22-1456".into(),
                None,
                true,
                Some(Utc::now()),
            ),
            VerifyPaymentBookingRecord::new(
                4,
                "Test-Kurs".into(),
                "Otto Normalverbraucher".into(),
                BigDecimal::from_str("45.90").unwrap(),
                "22-1467".into(),
                None,
                true,
                None,
            ),
        ];

        assert_eq!(
            compare_csv_with_bookings(csv, NaiveDate::from_ymd_opt(2022, 03, 11), &mut bookings),
            (
                HashMap::from([(4, "DE21500105179625862911".into())]),
                vec![
                    VerifyPaymentResult::new("1 bezahlte Buchung".into(), vec!["22-1467".into()]),
                    VerifyPaymentResult::new("0 Buchungen mit Problemen".into(), vec![]),
                    VerifyPaymentResult::new("0 nicht erkannte Buchungen".into(), vec![])
                ]
            )
        );

        assert_eq!(
            compare_csv_with_bookings(csv, NaiveDate::from_ymd_opt(2022, 03, 12), &mut bookings),
            (
                HashMap::new(),
                vec![
                    VerifyPaymentResult::new("0 bezahlte Buchungen".into(), vec![]),
                    VerifyPaymentResult::new("0 Buchungen mit Problemen".into(), vec![]),
                    VerifyPaymentResult::new("0 nicht erkannte Buchungen".into(), vec![])
                ]
            )
        );
    }

    fn compare_csv_with_bookings(
        csv: &str,
        csv_start_date: Option<NaiveDate>,
        bookings: &mut Vec<VerifyPaymentBookingRecord>,
    ) -> (HashMap<i32, String>, Vec<VerifyPaymentResult>) {
        let payment_records = read_payment_records(&csv, csv_start_date).unwrap();
        compare_payment_records_with_bookings(&payment_records, bookings).unwrap()
    }

    #[test]
    fn test_calculate_payday() {
        let booking_date = Utc::now();

        // event starts in 3 weeks
        let start_date = Utc::now() + Duration::weeks(3);
        assert_eq!(
            calculate_payday(&booking_date, &start_date, None),
            Utc::today() + Duration::weeks(1)
        );
        assert_eq!(
            calculate_payday(&booking_date, &start_date, Some(7)),
            Utc::today() + Duration::weeks(2)
        );
        assert_eq!(
            calculate_payday(&booking_date, &start_date, Some(0)),
            Utc::today() + Duration::weeks(3)
        );
        let tomorrow = Utc::today() + Duration::days(1);
        assert_eq!(
            calculate_payday(&booking_date, &start_date, Some(21)),
            tomorrow
        );
        assert_eq!(
            calculate_payday(&booking_date, &start_date, Some(28)),
            tomorrow
        );

        // event starts in 3 days
        let start_date = Utc::now() + Duration::days(3);
        assert_eq!(
            calculate_payday(&booking_date, &start_date, Some(1)),
            Utc::today() + Duration::days(2)
        );
        assert_eq!(
            calculate_payday(&booking_date, &start_date, Some(2)),
            tomorrow
        );
        assert_eq!(
            calculate_payday(&booking_date, &start_date, Some(3)),
            tomorrow
        );
        assert_eq!(
            calculate_payday(&booking_date, &start_date, Some(14)),
            tomorrow
        );

        // event starts today
        let start_date = Utc::now();
        assert_eq!(calculate_payday(&booking_date, &start_date, None), tomorrow);
        assert_eq!(
            calculate_payday(&booking_date, &start_date, Some(7)),
            tomorrow
        );

        // event started yesterday
        let start_date = Utc::now() - Duration::days(1);
        assert_eq!(calculate_payday(&booking_date, &start_date, None), tomorrow);
        assert_eq!(
            calculate_payday(&booking_date, &start_date, Some(7)),
            tomorrow
        );

        // negative paydays (because the bookings are in the past)
        let booking_date = Utc::now() - Duration::days(31);
        let start_date = Utc::now();
        assert_eq!(
            calculate_payday(&booking_date, &start_date, None),
            Utc::today() - Duration::days(14)
        );
        assert_eq!(
            calculate_payday(&booking_date, &start_date, Some(7)),
            Utc::today() - Duration::days(7)
        );
    }

    #[test]
    fn test_calc_due_in_days() {
        // without custom days
        let booking_date = Utc::now();
        let start_date = Utc::now() + Duration::days(28);
        assert_eq!(
            calc_due_in_days(booking_date, start_date, String::from("")).unwrap(),
            14
        );

        let booking_date = Utc::now();
        let start_date = Utc::now() + Duration::days(14);
        assert_eq!(
            calc_due_in_days(booking_date, start_date, String::from("")).unwrap(),
            1
        );

        let booking_date = Utc::now() - Duration::days(4);
        let start_date = Utc::now() + Duration::days(7);
        assert_eq!(
            calc_due_in_days(booking_date, start_date, String::from("{{payday}}")).unwrap(),
            -3
        );

        // with custom days
        let booking_date = Utc::now();
        let start_date = Utc::now() + Duration::days(7);
        assert_eq!(
            calc_due_in_days(booking_date, start_date, String::from("{{payday 7}}")).unwrap(),
            1
        );
        let booking_date = Utc::now() - Duration::days(31);
        let start_date = Utc::now() + Duration::days(7);
        assert_eq!(
            calc_due_in_days(booking_date, start_date, String::from("{{payday 1}}")).unwrap(),
            6
        );
        assert_eq!(
            calc_due_in_days(booking_date, start_date, String::from("{{payday 14}}")).unwrap(),
            -7
        );
    }
}
