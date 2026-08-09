use std::collections::{BTreeMap, HashMap, HashSet};

use anyhow::{Context, Result, anyhow, bail};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use chrono::{DateTime, Duration, NaiveDate, Utc};
use encoding::Encoding;
use encoding::{DecoderTrap, all::ISO_8859_1};
use lazy_static::lazy_static;
use lettre::message::SinglePart;
use regex::Regex;
use sqlx::PgPool;
use tracing::{error, info, warn};

use super::csv::PaymentRecord;
use super::{banking, template};
use crate::db::BookingResult;
use crate::email;
use crate::error::ValidationError;
use crate::logic::secrets::{SecretKey, SecretProvider};
use crate::models::{
    BookingResponse, Email, Event, EventBooking, EventCounter, EventCustomField, EventId,
    EventType, LifecycleStatus, MessageType, NewsSubscription, PartialEvent, PaymentMethod, ToEuro,
    UnpaidEventBooking, VerifyPaymentBookingRecord, VerifyPaymentResult,
};
use crate::{db, hashids};

pub(crate) mod notifications;

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
    db::get_events(
        pool,
        true,
        lifecycle_status_list,
        subscribers.unwrap_or(false),
    )
    .await
}

pub(crate) async fn get_all_custom_fields(
    pool: &sqlx::Pool<sqlx::Postgres>,
) -> Result<Vec<EventCustomField>> {
    let custom_fields = db::get_all_custom_fields(pool).await?;
    Ok(custom_fields)
}

pub(crate) async fn get_event_counters(pool: &PgPool, beta: bool) -> Result<Vec<EventCounter>> {
    db::get_event_counters(pool, into_lifecycle_status(beta)).await
}

pub(crate) async fn booking(
    pool: &PgPool,
    booking: EventBooking,
    email_gateway: &impl email::EmailGateway,
) -> BookingResponse {
    match book_event(pool, booking, email_gateway).await {
        Ok(response) => response,
        Err(e) => {
            if let Some(validation_err) = e.downcast_ref::<ValidationError>() {
                BookingResponse::failure(&validation_err.message)
            } else {
                error!("Booking failed: {:?}", e);
                BookingResponse::failure(MESSAGE_FAIL)
            }
        }
    }
}

pub(crate) async fn prebook_with_iban(
    pool: &PgPool,
    hash: &str,
    iban: String,
    email_gateway: &impl email::EmailGateway,
) -> Result<BookingResponse> {
    let normalized = banking::validate_iban_str(&iban)?;
    pre_book_event(pool, hash.to_string(), Some(normalized), email_gateway).await
}

pub(crate) async fn prebooking(
    pool: &PgPool,
    hash: String,
    email_gateway: &impl email::EmailGateway,
) -> BookingResponse {
    match pre_book_event(pool, hash, None, email_gateway).await {
        Ok(response) => response,
        Err(e) => {
            error!("Prebooking failed: {:?}", e);
            BookingResponse::failure(MESSAGE_FAIL)
        }
    }
}

pub(crate) async fn update(
    pool: &PgPool,
    partial_event: PartialEvent,
    email_gateway: &impl email::EmailGateway,
) -> Result<Event> {
    let (event, removed_dates) = db::write_event(pool, partial_event).await?;
    if let Some(removed_dates) = removed_dates
        && matches!(
            event.lifecycle_status,
            LifecycleStatus::Review | LifecycleStatus::Published | LifecycleStatus::Running
        )
    {
        let bookings = db::get_bookings(pool, &event.id, Some(true)).await?;
        if bookings.is_empty() {
            return Ok(event);
        }

        let subject = format!("{} Terminänderung {}", event.subject_prefix(), event.name);
        let template = match event.event_type {
            EventType::Fitness => include_str!("../../../templates/schedule_change_fitness.txt"),
            EventType::Events => include_str!("../../../templates/schedule_change_events.txt"),
        };

        let email_account = event.get_associated_email_account(email_gateway).await?;
        let message_type: MessageType = event.event_type.into();
        let mut messages = Vec::new();

        for (booking, _, _) in bookings {
            let body =
                template::render_schedule_change(template, &booking, &event, &removed_dates)?;

            messages.push(
                Email::new(message_type, booking.email, subject.clone(), body, None)
                    .into_message(&email_account, email_gateway)?,
            );
        }

        email_gateway
            .send_messages(&email_account, messages)
            .await?;
    }
    Ok(event)
}

pub(crate) async fn delete(pool: &PgPool, event_id: EventId) -> Result<()> {
    db::delete_event(pool, event_id).await
}

pub(crate) async fn verify_payments(
    pool: &PgPool,
    csv: String,
    csv_start_date: Option<NaiveDate>,
) -> Result<Vec<VerifyPaymentResult>> {
    let bytes = STANDARD
        .decode(&csv)
        .with_context(|| format!("Error decoding the cvs content: {}", csv))?;
    let csv = match ISO_8859_1.decode(&bytes, DecoderTrap::Strict) {
        Ok(value) => value,
        Err(e) => bail!("Decoding csv content with ISO 8859: {}", e.into_owned()),
    };

    let payment_records =
        tokio::task::spawn_blocking(move || read_payment_records(&csv, csv_start_date)).await??;
    let payment_ids = payment_records
        .iter()
        .flat_map(|r| &r.payment_ids)
        .collect::<HashSet<_>>();
    let mut bookings = db::get_bookings_to_verify_payment(pool, payment_ids).await?;
    let (verified_payments, result) =
        compare_payment_records_with_bookings(&payment_records, &mut bookings)?;
    if !verified_payments.is_empty() {
        db::mark_as_paid(pool, &verified_payments).await?;
    }

    Ok(result)
}

pub(crate) async fn get_unpaid_bookings(
    pool: &PgPool,
    event_type: EventType,
) -> Result<Vec<UnpaidEventBooking>> {
    let result = db::get_event_bookings_without_payment(pool, event_type).await?;

    let mut bookings = Vec::new();

    for (mut booking, booking_insert_date, first_event_date, event_template) in result.into_iter() {
        if let Some(first_event_date) = first_event_date {
            let booking_date;
            if let Some(payment_reminder_sent) = booking.payment_reminder_sent {
                booking_date = payment_reminder_sent
                    + Duration::try_days(3).with_context(|| "Cannot create duration of 3 days.")?;
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

/// calculate the days until the booking should be paid
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
    let payday = calculate_payday(&booking_date, &first_event_date, custom_days)?;

    // calculate due in days
    let due_in_days = payday - Utc::now().date_naive();

    Ok(due_in_days.num_days())
}

pub(crate) async fn update_payment(
    pool: &PgPool,
    booking_id: i32,
    update_payment: bool,
) -> Result<()> {
    db::update_payment(pool, booking_id, update_payment).await
}

pub(crate) async fn export_sepa_xml(
    pool: &PgPool,
    event_id: EventId,
    secrets: &dyn SecretProvider,
) -> Result<(String, String)> {
    use crate::models::SepaExportError;

    let event = db::get_event(pool, &event_id, false)
        .await?
        .ok_or_else(|| anyhow!("Event not found"))?;

    if event.payment_method != PaymentMethod::SepaDirectDebit {
        return Err(anyhow::Error::from(SepaExportError::NotASepaEvent));
    }

    let creditor_name = secrets.get(SecretKey::SepaCreditorName).await?;
    let creditor_iban = secrets.get(SecretKey::SepaCreditorIban).await?;
    let creditor_id = secrets.get(SecretKey::SepaCreditorId).await?;

    if creditor_name.is_empty() || creditor_iban.is_empty() || creditor_id.is_empty() {
        return Err(anyhow::Error::from(SepaExportError::ConfigIncomplete));
    }
    let bic_lookup = banking::HttpBicLookup::new();
    let creditor_bic = banking::lookup_bic(&bic_lookup, &creditor_iban)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to lookup creditor BIC: {}", e))?;

    let mut tx = pool.begin().await?;

    db::lock_sepa_eligible_bookings(&mut tx, event_id).await?;

    let bookings = db::get_sepa_eligible_bookings(&mut tx, event_id).await?;

    if bookings.is_empty() {
        return Err(anyhow::Error::from(SepaExportError::NoBookingsAvailable));
    }

    let mut booking_data = Vec::new();
    let mut failed_ibans = Vec::new();
    for sub in &bookings {
        match sub.iban.as_ref() {
            Some(iban) => match banking::lookup_bic(&bic_lookup, iban).await {
                Ok(bic) => booking_data.push((sub.clone(), bic)),
                Err(_) => failed_ibans.push(iban.clone()),
            },
            None => failed_ibans.push("(missing)".to_string()),
        }
    }

    if !failed_ibans.is_empty() {
        return Err(anyhow::Error::from(SepaExportError::BicLookupFailed(
            format!("BIC lookup failed for IBAN(s): {}", failed_ibans.join(", ")),
        )));
    }

    let xml = banking::generate_sepa_xml(
        &event,
        &booking_data,
        &creditor_name,
        &creditor_iban,
        &creditor_bic,
        &creditor_id,
    )?;

    let booking_ids: Vec<i32> = bookings.iter().map(|b| b.id).collect();
    db::mark_sepa_exported(&mut tx, &booking_ids).await?;

    tx.commit().await?;

    let filename = format!(
        "sepa-{}-{}.xml",
        event.name.replace(' ', "_").to_lowercase(),
        Utc::now().format("%Y-%m-%d"),
    );

    Ok((filename, xml))
}

pub(crate) async fn cancel_booking(
    pool: &PgPool,
    booking_id: i32,
    email_gateway: &impl email::EmailGateway,
) -> Result<()> {
    let (event, canceled_booking, waiting_list_booking) =
        db::cancel_event_booking(pool, booking_id).await?;

    let email_account = event.get_associated_email_account(email_gateway).await?;
    let mut messages = Vec::new();

    // create cancellation confirmation email
    let subject = format!("{} Stornierung Buchung", event.subject_prefix());
    let body = match event.event_type {
        EventType::Fitness => include_str!("../../../templates/cancel_booking_fitness.txt"),
        EventType::Events => include_str!("../../../templates/cancel_booking_events.txt"),
    };
    let body = template::render_booking(body, &canceled_booking, &event, None, None, None)?;
    messages.push(
        email_gateway
            .build_message(&email_account)?
            .to(canceled_booking.email.parse()?)
            .bcc(email_account.address.parse()?)
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
            email_gateway
                .build_message(&email_account)?
                .to(new_booking.email.parse()?)
                .bcc(email_account.address.parse()?)
                .subject(subject)
                .singlepart(SinglePart::plain(body))?,
        );
    }

    email_gateway
        .send_messages(&email_account, messages)
        .await?;

    Ok(())
}

/// Check that an event has finished. If so, move the event to status 'Finished',
/// send the attendee confirmation email, and move the event to status 'Closed'.
pub(crate) async fn close_finished_running_events(
    pool: &PgPool,
    email_gateway: &impl email::EmailGateway,
) -> Result<usize> {
    let mut count = 0;

    for event_id in db::get_all_finished_event_ids(pool).await? {
        // move event into status finsihed
        let event = update(
            pool,
            PartialEvent {
                id: Some(event_id),
                lifecycle_status: Some(LifecycleStatus::Finished),
                ..Default::default()
            },
            email_gateway,
        )
        .await?;

        // send confirmation emails for fitness events
        if matches!(event.event_type, EventType::Fitness) {
            notifications::send_participation_confirmation(pool, event_id, email_gateway).await?;
        }

        // move event into status closed
        update(
            pool,
            PartialEvent {
                id: Some(event_id),
                lifecycle_status: Some(LifecycleStatus::Closed),
                ..Default::default()
            },
            email_gateway,
        )
        .await?;

        count += 1;
    }

    Ok(count)
}

fn into_lifecycle_status(beta: bool) -> LifecycleStatus {
    if beta {
        LifecycleStatus::Review
    } else {
        LifecycleStatus::Published
    }
}

async fn book_event(
    pool: &PgPool,
    mut booking: EventBooking,
    email_gateway: &impl email::EmailGateway,
) -> Result<BookingResponse> {
    let event = db::get_event(pool, &booking.event_id, false)
        .await?
        .ok_or_else(|| anyhow!("Event not found"))?;

    if event.payment_method == PaymentMethod::SepaDirectDebit {
        let raw_iban = booking.iban.as_ref().ok_or_else(|| {
            warn!(
                "IBAN missing for SEPA booking of event {}",
                booking.event_id
            );
            ValidationError::new("Bitte gib eine gültige IBAN ein.")
        })?;
        let normalized = banking::validate_iban_str(raw_iban)?;
        booking.iban = Some(normalized);
    } else {
        booking.iban = None;
    }

    let price_multiplier = event.price_relevant_multiplier(&booking.custom_values);
    if event.custom_fields.iter().any(|cf| cf.price_relevant) && price_multiplier.is_none() {
        bail!(ValidationError::new(
            "Bitte gib eine gültige Anzahl ein.".to_string()
        ));
    }

    let booking_result = db::book_event(pool, &booking).await?;
    let booking_response = match booking_result {
        BookingResult::Booked(event, counter, payment_id) => {
            process_booking(
                pool,
                &booking,
                event,
                counter,
                true,
                payment_id,
                email_gateway,
            )
            .await?
        }
        BookingResult::WaitingList(event, counter, payment_id) => {
            process_booking(
                pool,
                &booking,
                event,
                counter,
                false,
                payment_id,
                email_gateway,
            )
            .await?
        }
        BookingResult::DuplicateBooking => {
            info!(
                "Event ({}) booking failed because a duplicate booking has been detected.",
                booking.event_id
            );
            BookingResponse::failure(
                "Wir haben schon eine Buchung mit diesen Anmeldedaten erkannt. Bitte verwende für weitere Buchungen andere Anmeldedaten.",
            )
        }
        BookingResult::NotBookable => {
            info!(
                "Event ({}) booking failed because the event is in a unbookable state.",
                booking.event_id
            );
            BookingResponse::failure("Das Event kann aktuell leider nicht gebucht werden.")
        }
        BookingResult::BookedOut => {
            info!(
                "Booking failed because Event ({}) was overbooked.",
                booking.event_id
            );
            BookingResponse::failure(MESSAGE_FAIL)
        }
    };

    Ok(booking_response)
}

async fn pre_book_event(
    pool: &PgPool,
    hash: String,
    provided_iban: Option<String>,
    email_gateway: &impl email::EmailGateway,
) -> Result<BookingResponse> {
    let ids = match hashids::decode(&hash) {
        Ok(ids) => ids,
        Err(e) => {
            warn!(
                "Prebooking failed because hash '{}' could not be decoded: {:?}",
                hash, e
            );
            return Ok(BookingResponse::failure("Ungültiger Buchungscode."));
        }
    };

    if ids.len() != 2 {
        bail!(
            "Booking failed because decoded prebooking hash ({}) has an invalid length: {}",
            hash,
            ids.len()
        );
    }

    let event_id = EventId::from(i32::try_from(ids[0])?);
    let subscriber_id: i32 = ids[1].try_into()?;

    let event = db::get_event(pool, &event_id, false)
        .await?
        .ok_or_else(|| anyhow!("Event not found"))?;

    if !event.lifecycle_status.is_bookable() {
        return Ok(BookingResponse::failure(
            "Diese Veranstaltung ist aktuell nicht buchbar.",
        ));
    }

    let mut iban = provided_iban;
    if event.payment_method == PaymentMethod::SepaDirectDebit && iban.is_none() {
        let prior_iban = db::find_prior_sepa_iban(pool, subscriber_id).await?;
        if let Some(prior) = prior_iban {
            iban = Some(prior);
        } else {
            return Ok(BookingResponse::requires_iban("Bitte gib deine IBAN ein."));
        }
    }

    let (booking_result, booking) = db::pre_book_event(pool, event_id, subscriber_id, iban).await?;
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
                email_gateway,
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
                email_gateway,
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
    email_gateway: &impl email::EmailGateway,
) -> Result<BookingResponse> {
    subscribe_to_updates(pool, booking, &event, email_gateway).await?;
    send_booking_mail(booking, &event, booked, payment_id, email_gateway).await?;
    info!("Booking of Event {} was successfull", booking.event_id);
    let message = if booked {
        "Die Buchung war erfolgreich. Du bekommst in den nächsten Minuten eine Bestätigung per E-Mail."
    } else {
        "Du stehst jetzt auf der Warteliste. Wir benachrichtigen Dich, wenn Plätze frei werden."
    };
    Ok(BookingResponse::success(message, counter))
}

async fn subscribe_to_updates(
    pool: &PgPool,
    booking: &EventBooking,
    event: &Event,
    email_gateway: &impl email::EmailGateway,
) -> Result<()> {
    // only subscribe to updates if updates field is true
    if !booking.updates.unwrap_or(false) {
        return Ok(());
    }
    let subscription = NewsSubscription::new(booking.email.clone(), vec![event.event_type.into()]);
    super::news::subscribe_to_news(pool, subscription, false, email_gateway).await?;

    Ok(())
}

async fn send_booking_mail(
    booking: &EventBooking,
    event: &Event,
    booked: bool,
    payment_id: String,
    email_gateway: &impl email::EmailGateway,
) -> Result<()> {
    let email_account = event.get_associated_email_account(email_gateway).await?;
    let subject;
    let template: &str;
    let opt_payment_id;
    if booked {
        subject = format!("{} Bestätigung Buchung", event.subject_prefix());
        template = &event.booking_template;
        opt_payment_id = Some(payment_id);
    } else {
        subject = format!("{} Bestätigung Warteliste", event.subject_prefix());
        template = match event.event_type {
            EventType::Fitness => include_str!("../../../templates/waiting_list_fitness.txt"),
            EventType::Events => include_str!("../../../templates/waiting_list_events.txt"),
        };
        opt_payment_id = None;
    }

    let mut body =
        template::render_booking(template, booking, event, opt_payment_id, None, Some(true))?;

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

    let message = email_gateway
        .build_message(&email_account)?
        .to(booking.email.parse()?)
        .bcc(email_account.address.parse()?)
        .subject(subject)
        .singlepart(SinglePart::plain(body))?;

    email_gateway
        .send_messages(&email_account, vec![message])
        .await?;

    Ok(())
}

pub(crate) fn create_prebooking_link(
    event_type: EventType,
    event_id: EventId,
    subscriber_id: i32,
) -> Result<String> {
    let mut url = String::from("https://www.sv-eutingen.de/");
    url.push_str(match event_type {
        EventType::Fitness => "fitness",
        EventType::Events => "events",
    });
    url.push_str("?code=");

    // create the code
    url.push_str(&hashids::encode(&[
        event_id.into_inner().try_into()?,
        subscriber_id.try_into()?,
    ]));

    Ok(url)
}

fn read_payment_records(
    csv: &str,
    csv_start_date: Option<NaiveDate>,
) -> Result<Vec<PaymentRecord>> {
    let mut records = Vec::new();

    for record in super::csv::read_payment_records(csv)? {
        // skip all records that are older than the start date
        if let Some(start_date) = csv_start_date
            && record.date < start_date
        {
            continue;
        }
        records.push(record);
    }

    Ok(records)
}

fn compare_payment_records_with_bookings(
    payment_records: &Vec<PaymentRecord>,
    bookings: &mut [VerifyPaymentBookingRecord],
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
        if payment_record.payment_ids.is_empty() {
            non_matching_payment_records.push(format!(
                "{} / {} / {} / {}",
                payment_record.payee,
                payment_record.payee_iban,
                payment_record.purpose,
                payment_record.volumne.to_euro()
            ));
            continue;
        }

        // Try to match each payment_id individually
        let mut matched_bookings = Vec::new();
        let mut missing_ids = Vec::new();
        for payment_id in &payment_record.payment_ids {
            if let Some(booking) = bookings.remove(payment_id) {
                matched_bookings.push((payment_id, booking));
            } else {
                missing_ids.push(payment_id.to_owned());
            }
        }

        // Report missing IDs, but do not skip processing matched ones
        if !missing_ids.is_empty() {
            non_matching_payment_records.push(format!(
                "Nicht erkannte Buchung(en): {} / {} / {} / {} / fehlende ID(s): {}",
                payment_record.payee,
                payment_record.payee_iban,
                payment_record.purpose,
                payment_record.volumne.to_euro(),
                missing_ids.join(", ")
            ));
        }

        // Check for errors in individual bookings and collect valid ones
        let mut error_ids = HashSet::new();
        let mut error_total = false;
        for (payment_id, booking) in &matched_bookings {
            if booking.payment_confirmed_at.is_some() {
                payment_bookings_with_errors
                    .entry(payment_id.to_owned())
                    .or_insert_with(Vec::new)
                    .push("Doppelt bezahlt: Buchung ist schon als bezahlt markiert".into());
                error_ids.insert(payment_id.to_owned());
            }
            if booking.enrolled && booking.canceled.is_some() {
                payment_bookings_with_errors
                    .entry(payment_id.to_owned())
                    .or_insert_with(Vec::new)
                    .push("Falsch bezahlt: Buchung ist als storniert markiert".into());
                error_ids.insert(payment_id.to_owned());
            }
            if !booking.enrolled {
                payment_bookings_with_errors
                    .entry(payment_id.to_owned())
                    .or_insert_with(Vec::new)
                    .push("Falsch bezahlt: Buchung ist von auf der Warteliste".into());
                error_ids.insert(payment_id.to_owned());
            }
        }

        // Check if sum of booking prices matches payment_record.volumne
        let total_booking_price = matched_bookings
            .iter()
            .map(|(_, b)| b.price.clone())
            .fold(bigdecimal::BigDecimal::from(0), |acc, x| acc + x);
        let record_volumne = payment_record.volumne.clone();
        if total_booking_price.clone() != record_volumne.clone() {
            for (payment_id, _) in &matched_bookings {
                payment_bookings_with_errors
                    .entry(payment_id.to_owned())
                    .or_insert_with(Vec::new)
                    .push(format!(
                        "Betrag falsch: erwartet {} != überwiesen {}",
                        total_booking_price.to_euro(),
                        record_volumne.to_euro()
                    ));
                error_ids.insert(payment_id.to_owned());
            }
            error_total = true;
        }

        // Mark only correct bookings as verified
        for (payment_id, booking) in matched_bookings {
            if !error_ids.contains(payment_id) && !error_total {
                verified_payment_bookings.push(booking);
                verified_ibans.insert(booking.booking_id, payment_record.payee_iban.clone());
            }
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
) -> Result<NaiveDate> {
    // default value is 14 days
    let mut days = 14;

    // overwrite with custom days - if available
    if let Some(custom_days) = custom_days {
        days = custom_days;
    }

    // calculated payday
    let mut payday = first_event_date.date_naive()
        - Duration::try_days(days)
            .with_context(|| format!("Cannot create duration of {days} days."))?;

    // override the payday if it is before the day after the booking date
    let earliest_paypay = booking_date.date_naive()
        + Duration::try_days(1).with_context(|| "Cannot create duration of 1 day.")?;
    if payday < earliest_paypay {
        payday = earliest_paypay
    }

    Ok(payday)
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use bigdecimal::{BigDecimal, FromPrimitive};
    use chrono::{NaiveDate, Utc};
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn test_create_prebooking_link() {
        assert_eq!(
            create_prebooking_link(EventType::Fitness, 1.into(), 0).unwrap(),
            format!(
                "https://www.sv-eutingen.de/fitness?code={}",
                hashids::encode(&[1, 0])
            )
        );
        assert_eq!(
            create_prebooking_link(EventType::Events, 2.into(), 1).unwrap(),
            format!(
                "https://www.sv-eutingen.de/events?code={}",
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
11.03.2022;11.03.2022;Überweisung;801;Familie Schmidt;0;DE12345678901234567890;12345678;BANKXYZ;22-2001,22-2002;;EUR;54,00;H
11.03.2022;11.03.2022;Überweisung;801;Familie Müller;0;DE09876543210987654321;87654321;BANKABC;22-3001,22-3002;;EUR;40,00;H
11.03.2022;11.03.2022;Überweisung;801;Familie Klein;0;DE11223344556677889900;11223344;BANKDEF;22-4001,22-4002;;EUR;27,00;H
11.03.2022;11.03.2022;Überweisung;801;Familie Doppel;0;DE44556677889900112233;44556677;BANKGHI;22-5001,22-5001;;EUR;27,00;H
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
            VerifyPaymentBookingRecord::new(
                10,
                "Test-Kurs".into(),
                "Anna Schmidt".into(),
                BigDecimal::from_i8(27).unwrap(),
                "22-2001".into(),
                None,
                true,
                None,
            ),
            VerifyPaymentBookingRecord::new(
                11,
                "Test-Kurs".into(),
                "Ben Schmidt".into(),
                BigDecimal::from_i8(27).unwrap(),
                "22-2002".into(),
                None,
                true,
                None,
            ),
            VerifyPaymentBookingRecord::new(
                12,
                "Test-Kurs".into(),
                "Clara Müller".into(),
                BigDecimal::from_i8(27).unwrap(),
                "22-3001".into(),
                None,
                true,
                None,
            ),
            VerifyPaymentBookingRecord::new(
                13,
                "Test-Kurs".into(),
                "David Müller".into(),
                BigDecimal::from_i8(27).unwrap(),
                "22-3002".into(),
                None,
                true,
                None,
            ),
            VerifyPaymentBookingRecord::new(
                14,
                "Test-Kurs".into(),
                "Eva Klein".into(),
                BigDecimal::from_i8(27).unwrap(),
                "22-4001".into(),
                None,
                true,
                None,
            ),
            VerifyPaymentBookingRecord::new(
                15,
                "Test-Kurs".into(),
                "Felix Doppel".into(),
                BigDecimal::from_i8(27).unwrap(),
                "22-5001".into(),
                None,
                true,
                None,
            ),
        ];

        assert_eq!(
            compare_csv_with_bookings(csv, NaiveDate::from_ymd_opt(2022, 3, 11), &mut bookings),
            (
                HashMap::from([
                    (4, "DE21500105179625862911".into()),
                    (10, "DE12345678901234567890".into()),
                    (11, "DE12345678901234567890".into()),
                    (14, "DE11223344556677889900".into()),
                    (15, "DE44556677889900112233".into()),
                ]),
                vec![
                    VerifyPaymentResult::new(
                        "5 bezahlte Buchungen".into(),
                        vec![
                            "22-1467".into(),
                            "22-2001".into(),
                            "22-2002".into(),
                            "22-4001".into(),
                            "22-5001".into()
                        ]
                    ),
                    VerifyPaymentResult::new(
                        "2 Buchungen mit Problemen".into(),
                        vec![
                            "22-3001 / Betrag falsch: erwartet 54,00 € != überwiesen 40,00 €"
                                .into(),
                            "22-3002 / Betrag falsch: erwartet 54,00 € != überwiesen 40,00 €"
                                .into()
                        ]
                    ),
                    VerifyPaymentResult::new(
                        "1 nicht erkannte Buchung".into(),
                        vec![
                            "Nicht erkannte Buchung(en): Familie Klein / DE11223344556677889900 / 22-4001,22-4002 / 27,00 € / fehlende ID(s): 22-4002".into()
                        ]
                    )
                ]
            )
        );

        assert_eq!(
            compare_csv_with_bookings(csv, NaiveDate::from_ymd_opt(2022, 3, 12), &mut bookings),
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
        bookings: &mut [VerifyPaymentBookingRecord],
    ) -> (HashMap<i32, String>, Vec<VerifyPaymentResult>) {
        let payment_records = read_payment_records(csv, csv_start_date).unwrap();
        compare_payment_records_with_bookings(&payment_records, bookings).unwrap()
    }

    #[test]
    fn test_calculate_payday() {
        let booking_date = Utc::now();

        // event starts in 3 weeks
        let start_date = Utc::now() + Duration::try_weeks(3).unwrap();
        assert_eq!(
            calculate_payday(&booking_date, &start_date, None).unwrap(),
            Utc::now().date_naive() + Duration::try_weeks(1).unwrap()
        );
        assert_eq!(
            calculate_payday(&booking_date, &start_date, Some(7)).unwrap(),
            Utc::now().date_naive() + Duration::try_weeks(2).unwrap()
        );
        assert_eq!(
            calculate_payday(&booking_date, &start_date, Some(0)).unwrap(),
            Utc::now().date_naive() + Duration::try_weeks(3).unwrap()
        );
        let tomorrow = Utc::now().date_naive() + Duration::try_days(1).unwrap();
        assert_eq!(
            calculate_payday(&booking_date, &start_date, Some(21)).unwrap(),
            tomorrow
        );
        assert_eq!(
            calculate_payday(&booking_date, &start_date, Some(28)).unwrap(),
            tomorrow
        );

        // event starts in 3 days
        let start_date = Utc::now() + Duration::try_days(3).unwrap();
        assert_eq!(
            calculate_payday(&booking_date, &start_date, Some(1)).unwrap(),
            Utc::now().date_naive() + Duration::try_days(2).unwrap()
        );
        assert_eq!(
            calculate_payday(&booking_date, &start_date, Some(2)).unwrap(),
            tomorrow
        );
        assert_eq!(
            calculate_payday(&booking_date, &start_date, Some(3)).unwrap(),
            tomorrow
        );
        assert_eq!(
            calculate_payday(&booking_date, &start_date, Some(14)).unwrap(),
            tomorrow
        );

        // event starts today
        let start_date = Utc::now();
        assert_eq!(
            calculate_payday(&booking_date, &start_date, None).unwrap(),
            tomorrow
        );
        assert_eq!(
            calculate_payday(&booking_date, &start_date, Some(7)).unwrap(),
            tomorrow
        );

        // event started yesterday
        let start_date = Utc::now() - Duration::try_days(1).unwrap();
        assert_eq!(
            calculate_payday(&booking_date, &start_date, None).unwrap(),
            tomorrow
        );
        assert_eq!(
            calculate_payday(&booking_date, &start_date, Some(7)).unwrap(),
            tomorrow
        );

        // negative paydays (because the bookings are in the past)
        let booking_date = Utc::now() - Duration::try_days(31).unwrap();
        let start_date = Utc::now();
        assert_eq!(
            calculate_payday(&booking_date, &start_date, None).unwrap(),
            Utc::now().date_naive() - Duration::try_days(14).unwrap()
        );
        assert_eq!(
            calculate_payday(&booking_date, &start_date, Some(7)).unwrap(),
            Utc::now().date_naive() - Duration::try_days(7).unwrap()
        );
    }

    #[test]
    fn test_calc_due_in_days() {
        // without custom days
        let booking_date = Utc::now();
        let start_date = Utc::now() + Duration::try_days(28).unwrap();
        assert_eq!(
            calc_due_in_days(booking_date, start_date, String::from("")).unwrap(),
            14
        );

        let booking_date = Utc::now();
        let start_date = Utc::now() + Duration::try_days(14).unwrap();
        assert_eq!(
            calc_due_in_days(booking_date, start_date, String::from("")).unwrap(),
            1
        );

        let booking_date = Utc::now() - Duration::try_days(4).unwrap();
        let start_date = Utc::now() + Duration::try_days(7).unwrap();
        assert_eq!(
            calc_due_in_days(booking_date, start_date, String::from("{{payday}}")).unwrap(),
            -3
        );

        // with custom days
        let booking_date = Utc::now();
        let start_date = Utc::now() + Duration::try_days(7).unwrap();
        assert_eq!(
            calc_due_in_days(booking_date, start_date, String::from("{{payday 7}}")).unwrap(),
            1
        );
        let booking_date = Utc::now() - Duration::try_days(31).unwrap();
        let start_date = Utc::now() + Duration::try_days(7).unwrap();
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

#[cfg(test)]
async fn create_test_event(pool: &PgPool, status: LifecycleStatus) -> Result<Event> {
    use bigdecimal::BigDecimal;
    let now = Utc::now();
    let event = db::write_event(
        pool,
        PartialEvent {
            event_type: Some(EventType::Fitness),
            lifecycle_status: Some(status),
            name: Some("Test Event".to_string()),
            sort_index: Some(0),
            short_description: Some("Short desc".to_string()),
            description: Some("Full desc".to_string()),
            image: Some("test.png".to_string()),
            light: Some(true),
            dates: Some(vec![now + Duration::try_days(30).unwrap()]),
            duration_in_minutes: Some(60),
            max_subscribers: Some(10),
            max_waiting_list: Some(5),
            price_member: Some(BigDecimal::from(20)),
            price_non_member: Some(BigDecimal::from(25)),
            location: Some("Test Location".to_string()),
            booking_template: Some("Booking template".to_string()),
            payment_account: Some("DE1234".to_string()),
            external_operator: Some(false),
            ..Default::default()
        },
    )
    .await?;
    Ok(event.0)
}

#[cfg(test)]
mod events_integration_tests {
    use anyhow::Result;
    use bigdecimal::BigDecimal;
    use chrono::{Duration, Utc};
    use lettre::Message;
    use pretty_assertions::assert_eq;
    use sqlx::PgPool;

    use crate::logic::secrets::MockSecretProvider;
    use crate::models::{EventBooking, EventType, LifecycleStatus, PartialEvent, PaymentMethod};
    use crate::test_utils::mock_email_gateway;

    use super::*;

    fn make_booking(event_id: EventId) -> EventBooking {
        make_booking_with_values(event_id, vec![])
    }

    fn make_booking_with_values(event_id: EventId, custom_values: Vec<String>) -> EventBooking {
        EventBooking {
            event_id,
            first_name: "Max".to_string(),
            last_name: "Mustermann".to_string(),
            street: "Teststr 1".to_string(),
            city: "Teststadt".to_string(),
            email: "max@test.com".to_string(),
            phone: None,
            member: Some(true),
            updates: Some(false),
            comments: None,
            custom_values,
            token: None,
            iban: None,
        }
    }

    async fn weinwanderung_event(pool: &PgPool) -> Result<Event> {
        use crate::models::{EventCustomField, EventCustomFieldType};

        let cf_row = sqlx::query!(
            r#"INSERT INTO event_custom_fields (name, type, price_relevant)
               VALUES ('Anzahl', 'Number', true)
               RETURNING id"#
        )
        .fetch_one(pool)
        .await?;
        let cf = EventCustomField::new(
            cf_row.id,
            "Anzahl".to_string(),
            EventCustomFieldType::Number,
            None,
            None,
            true,
        );

        let event = db::write_event(
            pool,
            PartialEvent {
                event_type: Some(EventType::Events),
                lifecycle_status: Some(LifecycleStatus::Published),
                name: Some("Weinwanderung".to_string()),
                sort_index: Some(0),
                short_description: Some("Short".to_string()),
                description: Some("Full".to_string()),
                image: Some("test.png".to_string()),
                light: Some(true),
                dates: Some(vec![Utc::now() + Duration::try_days(30).unwrap()]),
                duration_in_minutes: Some(120),
                max_subscribers: Some(20),
                max_waiting_list: Some(5),
                price_member: Some(BigDecimal::from(25)),
                price_non_member: Some(BigDecimal::from(25)),
                location: Some("Weingut".to_string()),
                booking_template: Some("Hallo {{firstname}}, Preis: {{price}}".to_string()),
                payment_account: Some("DE1234".to_string()),
                external_operator: Some(false),
                custom_fields: Some(vec![cf]),
                ..Default::default()
            },
        )
        .await?;
        Ok(event.0)
    }

    #[sqlx::test]
    async fn test_booking_draft_event_not_bookable(pool: PgPool) {
        let event = create_test_event(&pool, LifecycleStatus::Draft)
            .await
            .unwrap();
        let mock_sender = mock_email_gateway(vec![]).0;
        let booking_data = make_booking(event.id);

        let response = super::booking(&pool, booking_data, &mock_sender).await;
        assert!(!response.success);
    }

    #[sqlx::test]
    async fn test_booking_published_event_not_bookable_due_to_capacity(pool: PgPool) {
        // Create event with 0 max_subscribers (capacity full)
        let event = db::write_event(
            &pool,
            PartialEvent {
                event_type: Some(EventType::Fitness),
                lifecycle_status: Some(LifecycleStatus::Published),
                name: Some("Full Event".to_string()),
                sort_index: Some(0),
                short_description: Some("Short".to_string()),
                description: Some("Desc".to_string()),
                image: Some("img.png".to_string()),
                light: Some(true),
                dates: Some(vec![Utc::now() + Duration::try_days(30).unwrap()]),
                duration_in_minutes: Some(60),
                max_subscribers: Some(0),
                max_waiting_list: Some(0),
                price_member: Some(BigDecimal::from(20)),
                price_non_member: Some(BigDecimal::from(25)),
                location: Some("Location".to_string()),
                booking_template: Some("Template".to_string()),
                payment_account: Some("DE1234".to_string()),
                external_operator: Some(false),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        let mock_sender = mock_email_gateway(vec![]).0;
        let booking_data = make_booking(event.0.id);
        let response = super::booking(&pool, booking_data, &mock_sender).await;
        assert!(!response.success);
    }

    #[sqlx::test]
    async fn test_prebooking_invalid_hash(pool: PgPool) {
        let mock_sender = mock_email_gateway(vec![]).0;
        let response = prebooking(&pool, "invalid_hash".to_string(), &mock_sender).await;
        assert!(!response.success);
    }

    #[sqlx::test]
    async fn test_cancel_nonexistent_booking(pool: PgPool) {
        let mock_sender = mock_email_gateway(vec![]).0;
        let result = cancel_booking(&pool, 99999, &mock_sender).await;
        assert!(result.is_err());
    }

    #[sqlx::test]
    async fn test_update_event(pool: PgPool) -> Result<()> {
        let event = create_test_event(&pool, LifecycleStatus::Draft).await?;
        let mock_sender = mock_email_gateway(vec![]).0;

        let updated = update(
            &pool,
            PartialEvent {
                id: Some(event.id),
                name: Some("Updated Name".to_string()),
                ..Default::default()
            },
            &mock_sender,
        )
        .await?;

        assert_eq!(updated.name, "Updated Name");
        Ok(())
    }

    #[sqlx::test]
    async fn test_update_event_with_removed_dates_no_bookings(pool: PgPool) -> Result<()> {
        let event = create_test_event(&pool, LifecycleStatus::Published).await?;
        let mock_sender = mock_email_gateway(vec![]).0;

        // Update with empty dates (removing all dates) - should not send emails since no bookings
        let updated = update(
            &pool,
            PartialEvent {
                id: Some(event.id),
                dates: Some(vec![]),
                ..Default::default()
            },
            &mock_sender,
        )
        .await?;

        assert_eq!(updated.name, event.name);
        Ok(())
    }

    #[sqlx::test]
    async fn test_send_booking_mail_booked(pool: PgPool) -> Result<()> {
        let event = create_test_event(&pool, LifecycleStatus::Published).await?;

        let booking = EventBooking {
            event_id: event.id,
            first_name: "Max".to_string(),
            last_name: "Mustermann".to_string(),
            street: "Teststr 1".to_string(),
            city: "Teststadt".to_string(),
            email: "max@test.com".to_string(),
            phone: None,
            member: Some(true),
            updates: Some(false),
            comments: None,
            custom_values: vec![],
            token: None,
            iban: None,
        };

        let mock_sender = mock_email_gateway(vec![(
            crate::models::EmailType::Fitness,
            "test@example.com",
        )])
        .0;

        send_booking_mail(&booking, &event, true, "PAY123".to_string(), &mock_sender).await?;

        Ok(())
    }

    #[sqlx::test]
    async fn test_send_booking_mail_waiting_list(pool: PgPool) -> Result<()> {
        let event = create_test_event(&pool, LifecycleStatus::Published).await?;

        let booking = EventBooking {
            event_id: event.id,
            first_name: "Max".to_string(),
            last_name: "Mustermann".to_string(),
            street: "Teststr 1".to_string(),
            city: "Teststadt".to_string(),
            email: "max@test.com".to_string(),
            phone: None,
            member: Some(true),
            updates: Some(false),
            comments: None,
            custom_values: vec![],
            token: None,
            iban: None,
        };

        let mock_sender = mock_email_gateway(vec![(
            crate::models::EmailType::Fitness,
            "test@example.com",
        )])
        .0;

        send_booking_mail(&booking, &event, false, "PAY123".to_string(), &mock_sender).await?;

        Ok(())
    }

    // ---- SEPA-related integration tests ----

    async fn create_test_event_with_payment_method(
        pool: &PgPool,
        status: LifecycleStatus,
        payment_method: PaymentMethod,
    ) -> Result<Event> {
        let now = Utc::now();
        let event = db::write_event(
            pool,
            PartialEvent {
                event_type: Some(EventType::Fitness),
                lifecycle_status: Some(status),
                name: Some("SEPA Test Event".to_string()),
                sort_index: Some(0),
                short_description: Some("Short desc".to_string()),
                description: Some("Full desc".to_string()),
                image: Some("test.png".to_string()),
                light: Some(true),
                dates: Some(vec![now + Duration::try_days(30).unwrap()]),
                duration_in_minutes: Some(60),
                max_subscribers: Some(10),
                max_waiting_list: Some(5),
                price_member: Some(BigDecimal::from(20)),
                price_non_member: Some(BigDecimal::from(25)),
                location: Some("Test Location".to_string()),
                booking_template: Some("Booking template".to_string()),
                payment_account: Some("DE1234".to_string()),
                external_operator: Some(false),
                payment_method: Some(payment_method),
                ..Default::default()
            },
        )
        .await?;
        Ok(event.0)
    }

    #[sqlx::test]
    async fn test_booking_sepa_event_with_valid_iban(pool: PgPool) -> Result<()> {
        let event = create_test_event_with_payment_method(
            &pool,
            LifecycleStatus::Published,
            PaymentMethod::SepaDirectDebit,
        )
        .await?;
        let mock_sender = mock_email_gateway(vec![(
            crate::models::EmailType::Fitness,
            "test@example.com",
        )])
        .0;

        let mut booking_data = make_booking(event.id);
        booking_data.iban = Some("DE89 3704 0044 0532 0130 00".to_string());

        let response = super::booking(&pool, booking_data, &mock_sender).await;
        assert!(response.success, "Booking should succeed with valid IBAN");

        let bookings = db::get_bookings(&pool, &event.id, None).await?;
        assert_eq!(bookings.len(), 1);
        assert_eq!(
            bookings[0].0.iban.as_deref(),
            Some("DE89370400440532013000"),
            "IBAN should be normalized"
        );

        Ok(())
    }

    #[sqlx::test]
    async fn test_booking_sepa_event_without_iban(pool: PgPool) -> Result<()> {
        let event = create_test_event_with_payment_method(
            &pool,
            LifecycleStatus::Published,
            PaymentMethod::SepaDirectDebit,
        )
        .await?;
        let mock_sender = mock_email_gateway(vec![]).0;

        let booking_data = make_booking(event.id);
        // iban is None by default

        let response = super::booking(&pool, booking_data, &mock_sender).await;
        assert!(!response.success, "Booking should fail without IBAN");

        Ok(())
    }

    #[sqlx::test]
    async fn test_booking_bank_transfer_event_clears_iban(pool: PgPool) -> Result<()> {
        let event = create_test_event_with_payment_method(
            &pool,
            LifecycleStatus::Published,
            PaymentMethod::BankTransfer,
        )
        .await?;
        let mock_sender = mock_email_gateway(vec![(
            crate::models::EmailType::Fitness,
            "test@example.com",
        )])
        .0;

        let mut booking_data = make_booking(event.id);
        booking_data.iban = Some("DE89370400440532013000".to_string());

        let response = super::booking(&pool, booking_data, &mock_sender).await;
        assert!(response.success, "Booking should succeed for BankTransfer");

        let bookings = db::get_bookings(&pool, &event.id, None).await?;
        assert_eq!(bookings.len(), 1);
        assert_eq!(
            bookings[0].0.iban, None,
            "IBAN should be cleared for BankTransfer events"
        );

        Ok(())
    }

    #[sqlx::test]
    async fn test_prebooking_sepa_without_iban_no_prior(pool: PgPool) -> Result<()> {
        let event = create_test_event_with_payment_method(
            &pool,
            LifecycleStatus::Published,
            PaymentMethod::SepaDirectDebit,
        )
        .await?;
        let mock_sender = mock_email_gateway(vec![]).0;

        // Insert a subscriber directly (no prior SEPA booking with IBAN)
        let subscriber_row = sqlx::query!(
            r#"INSERT INTO event_subscribers (first_name, last_name, street, city, email, phone, member)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING id"#,
            "Max",
            "Mustermann",
            "Teststr 1",
            "Teststadt",
            "max@test.com",
            None::<String>,
            true
        )
        .fetch_one(&pool)
        .await?;
        let subscriber_id = subscriber_row.id;

        let hash =
            crate::hashids::encode(&[event.id.into_inner().try_into()?, subscriber_id.try_into()?]);
        let response = prebooking(&pool, hash, &mock_sender).await;
        assert!(
            !response.success,
            "Prebooking should fail without IBAN and no prior booking"
        );
        let json = serde_json::to_string(&response).unwrap();
        assert!(
            json.contains("\"requires_iban\":true"),
            "Response should indicate IBAN is required, got: {}",
            json
        );

        Ok(())
    }

    #[sqlx::test]
    async fn test_prebooking_sepa_without_iban_with_prior(pool: PgPool) -> Result<()> {
        let event1 = create_test_event_with_payment_method(
            &pool,
            LifecycleStatus::Published,
            PaymentMethod::SepaDirectDebit,
        )
        .await?;
        let (mock_sender, _) = mock_email_gateway(vec![(
            crate::models::EmailType::Fitness,
            "test@example.com",
        )]);

        // First booking with IBAN to establish subscriber
        let mut booking_data = make_booking(event1.id);
        booking_data.iban = Some("DE89370400440532013000".to_string());
        let response = super::booking(&pool, booking_data, &mock_sender).await;
        assert!(response.success, "First booking should succeed");

        let bookings1 = db::get_bookings(&pool, &event1.id, None).await?;
        let subscriber_id = bookings1[0].1;

        // Second SEPA event
        let event2 = create_test_event_with_payment_method(
            &pool,
            LifecycleStatus::Published,
            PaymentMethod::SepaDirectDebit,
        )
        .await?;

        let hash = crate::hashids::encode(&[
            event2.id.into_inner().try_into()?,
            subscriber_id.try_into()?,
        ]);
        let response = prebooking(&pool, hash, &mock_sender).await;
        assert!(
            response.success,
            "Prebooking should succeed using prior IBAN"
        );

        let bookings2 = db::get_bookings(&pool, &event2.id, None).await?;
        assert_eq!(bookings2.len(), 1);
        assert_eq!(
            bookings2[0].0.iban.as_deref(),
            Some("DE89370400440532013000"),
            "Prior IBAN should be reused"
        );

        Ok(())
    }

    #[sqlx::test]
    async fn test_prebook_with_iban_valid(pool: PgPool) -> Result<()> {
        let event1 = create_test_event_with_payment_method(
            &pool,
            LifecycleStatus::Published,
            PaymentMethod::SepaDirectDebit,
        )
        .await?;
        let (mock_sender, _) = mock_email_gateway(vec![(
            crate::models::EmailType::Fitness,
            "test@example.com",
        )]);

        // First booking to establish subscriber
        let mut booking_data = make_booking(event1.id);
        booking_data.iban = Some("DE89370400440532013000".to_string());
        let response = super::booking(&pool, booking_data, &mock_sender).await;
        assert!(response.success);

        let bookings1 = db::get_bookings(&pool, &event1.id, None).await?;
        let subscriber_id = bookings1[0].1;

        // Second SEPA event
        let event2 = create_test_event_with_payment_method(
            &pool,
            LifecycleStatus::Published,
            PaymentMethod::SepaDirectDebit,
        )
        .await?;

        let hash = crate::hashids::encode(&[
            event2.id.into_inner().try_into()?,
            subscriber_id.try_into()?,
        ]);
        let response = prebook_with_iban(
            &pool,
            &hash,
            "DE89 3704 0044 0532 0130 00".to_string(),
            &mock_sender,
        )
        .await?;
        assert!(
            response.success,
            "Prebooking with valid IBAN should succeed"
        );

        let bookings2 = db::get_bookings(&pool, &event2.id, None).await?;
        assert_eq!(bookings2.len(), 1);
        assert_eq!(
            bookings2[0].0.iban.as_deref(),
            Some("DE89370400440532013000"),
            "Provided IBAN should be normalized and stored"
        );

        Ok(())
    }

    #[sqlx::test]
    async fn test_export_sepa_xml_non_sepa_event(pool: PgPool) -> Result<()> {
        let event = create_test_event_with_payment_method(
            &pool,
            LifecycleStatus::Published,
            PaymentMethod::BankTransfer,
        )
        .await?;

        // No expectations on the mock: the non-SEPA path returns before reading
        // any secrets, so calling `get` would panic the test.
        let result = export_sepa_xml(&pool, event.id, &MockSecretProvider::new()).await;
        assert!(result.is_err(), "Should fail for non-SEPA events");
        let err = result.unwrap_err();
        let sepa_err = err.downcast_ref::<crate::models::SepaExportError>();
        assert!(
            matches!(
                sepa_err,
                Some(crate::models::SepaExportError::NotASepaEvent)
            ),
            "Expected NotASepaEvent, got: {:?}",
            sepa_err
        );

        Ok(())
    }

    #[sqlx::test]
    async fn test_export_sepa_xml_config_incomplete(pool: PgPool) -> Result<()> {
        let event = create_test_event_with_payment_method(
            &pool,
            LifecycleStatus::Published,
            PaymentMethod::SepaDirectDebit,
        )
        .await?;

        // Mock returns empty creditor config. Runs with no env vars set and no
        // AWS access — the SecretProvider seam lets us exercise the SEPA path
        // without touching the outside world.
        let mut mock_secrets = MockSecretProvider::new();
        mock_secrets
            .expect_get()
            .returning(|_| Box::pin(async { Ok(String::new()) }));

        let result = export_sepa_xml(&pool, event.id, &mock_secrets).await;
        assert!(result.is_err(), "Should fail with incomplete SEPA config");
        let err = result.unwrap_err();
        let sepa_err = err.downcast_ref::<crate::models::SepaExportError>();
        assert!(
            matches!(
                sepa_err,
                Some(crate::models::SepaExportError::ConfigIncomplete)
            ),
            "Expected ConfigIncomplete, got: {:?}",
            sepa_err
        );

        Ok(())
    }

    #[sqlx::test]
    async fn test_booking_price_relevant_custom_field(pool: PgPool) -> Result<()> {
        use crate::models::EmailType;

        let event = weinwanderung_event(&pool).await?;

        // Book with custom_values = ["3"] → total = 25 × 3 = 75
        let booking = make_booking_with_values(event.id, vec!["3".to_string()]);

        let (mock_sender, captured) =
            mock_email_gateway(vec![(EmailType::Events, "test@example.com")]);

        let response = super::booking(&pool, booking, &mock_sender).await;
        assert!(response.success, "Booking should succeed");

        // Verify confirmation email shows 75,00 € (25 × 3)
        let messages: Vec<Message> = captured
            .lock()
            .unwrap()
            .iter()
            .flat_map(|(_, msgs)| msgs)
            .cloned()
            .collect();
        assert_eq!(messages.len(), 1, "One confirmation email should be sent");
        let formatted = messages[0].formatted();
        let body = String::from_utf8_lossy(&formatted);
        assert!(
            body.contains("75,00"),
            "Email body should show 75,00 € — got: {body}"
        );

        // Verify booking persisted with custom_value_1 = "3"
        let persisted = sqlx::query!(
            r#"SELECT custom_value_1 FROM v_event_bookings WHERE event_id = $1 LIMIT 1"#,
            event.id.get_ref()
        )
        .fetch_one(&pool)
        .await?;
        assert_eq!(persisted.custom_value_1.as_deref(), Some("3"));

        Ok(())
    }

    #[sqlx::test]
    async fn test_booking_price_relevant_custom_field_validation(pool: PgPool) -> Result<()> {
        let event = weinwanderung_event(&pool).await?;

        // Missing value for the price-relevant field: rejected before any email is
        // sent. An empty mock has no expectations, so any email call would panic —
        // proving the validation branch fires before the confirmation-email path.
        let missing = make_booking_with_values(event.id, vec![]);
        let response = super::booking(&pool, missing, &mock_email_gateway(vec![]).0).await;
        assert!(
            !response.success,
            "Booking with a missing price-relevant value should be rejected"
        );

        // Non-numeric value: also rejected before any email is sent.
        let non_numeric = make_booking_with_values(event.id, vec!["abc".to_string()]);
        let response = super::booking(&pool, non_numeric, &mock_email_gateway(vec![]).0).await;
        assert!(
            !response.success,
            "Booking with a non-numeric price-relevant value should be rejected"
        );

        Ok(())
    }
}
