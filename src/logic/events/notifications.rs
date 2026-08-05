use std::collections::hash_map::Entry;
use std::collections::HashMap;

use anyhow::{Result, anyhow, bail};
use chrono::Locale;
use lettre::message::header::ContentType;
use lettre::message::{Attachment, MultiPart, SinglePart};
use sqlx::PgPool;

use crate::db;
use crate::email;
use crate::logic::{export, template};
use crate::models::{Email, EventEmail, EventId, EventType, MessageType, ToEuro};

pub(crate) async fn send_event_email(
    pool: &PgPool,
    data: EventEmail,
    email_gateway: &impl email::EmailGateway,
) -> Result<()> {
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

    let bookings = db::get_bookings(pool, &data.event_id, enrolled).await?;
    if bookings.is_empty() {
        return Ok(());
    }

    let event_id = match &data.prebooking_event_id {
        Some(event_id) => event_id,
        None => &data.event_id,
    };

    let event = db::get_event(pool, event_id, false)
        .await?
        .ok_or_else(|| anyhow!("Found no event with id '{}'", event_id))?;

    let email_account = event.get_associated_email_account(email_gateway).await?;
    let message_type: MessageType = event.event_type.into();
    let mut messages = Vec::new();

    for (booking, subscriber_id, payment_id) in bookings {
        let prebooking_link;
        if let Some(event_id) = data.prebooking_event_id {
            prebooking_link = Some(super::create_prebooking_link(
                event.event_type,
                event_id,
                subscriber_id,
            )?);
        } else {
            prebooking_link = None;
        }

        let body = template::render_booking(
            &data.body,
            &booking,
            &event,
            Some(payment_id),
            prebooking_link,
            None,
        )?;

        let attachments = data
            .attachments
            .as_ref()
            .map(|attachments| attachments.to_vec());

        messages.push(
            Email::new(
                message_type,
                booking.email,
                data.subject.clone(),
                body,
                attachments,
            )
            .into_message(&email_account, email_gateway)?,
        );
    }

    email_gateway
        .send_messages(&email_account, messages)
        .await?;

    Ok(())
}

pub(crate) async fn send_event_reminders(
    pool: &PgPool,
    email_gateway: &impl email::EmailGateway,
) -> Result<usize> {
    let events = db::get_reminder_events(pool).await?;

    for event in &events {
        let email_account = event.get_associated_email_account(email_gateway).await?;
        let message_type: MessageType = event.event_type.into();
        let mut messages = Vec::new();

        let (subject, body) = match event.event_type {
            EventType::Fitness => (
                format!("{} Info zum Kursstart", event.subject_prefix()),
                include_str!("../../../templates/event_reminder_fitness.txt"),
            ),
            EventType::Events => (
                format!("{} Info zum Eventstart", event.subject_prefix()),
                include_str!("../../../templates/event_reminder_events.txt"),
            ),
        };

        if let Some(subscribers) = &event.subscribers {
            for subscriber in subscribers.iter().filter(|s| s.enrolled) {
                let body = template::render_event_reminder(body, event, subscriber)?;

                messages.push(
                    Email::new(
                        message_type,
                        subscriber.email.clone(),
                        subject.clone(),
                        body,
                        None,
                    )
                    .into_message(&email_account, email_gateway)?,
                );
            }

            email_gateway
                .send_messages(&email_account, messages)
                .await?;

            db::mark_as_reminder_sent(pool, &event.id).await?;
        }
    }

    Ok(events.len())
}

pub(crate) async fn send_payment_reminders(
    pool: &PgPool,
    event_type: EventType,
    email_gateway: &impl email::EmailGateway,
) -> Result<usize> {
    let bookings = super::get_unpaid_bookings(pool, event_type)
        .await?
        .into_iter()
        .filter(|booking| matches!(booking.due_in_days, Some(due_in_days) if due_in_days < 0))
        .collect::<Vec<_>>();

    let email_account = email_gateway.account_by_type(event_type.into()).await?;
    let message_type: MessageType = event_type.into();
    let mut messages = Vec::new();

    let subject = format!("{} Zahlungserinnerung", event_type.subject_prefix());
    let body = match event_type {
        EventType::Fitness => include_str!("../../../templates/payment_reminder_fitness.txt"),
        EventType::Events => include_str!("../../../templates/payment_reminder_events.txt"),
    };

    let mut event_cache = HashMap::new();
    for booking in bookings.iter() {
        let key = booking.event_id;
        if let Entry::Vacant(e) = event_cache.entry(key) {
            let value = db::get_event(pool, &booking.event_id, false)
                .await?
                .ok_or_else(|| anyhow!("Event with id '{}' is missing", key))?;
            e.insert(value);
        }
        let event = event_cache
            .get(&key)
            .ok_or_else(|| anyhow!("Event with id '{}' is not in the cache", key))?;

        let body = template::render_payment_reminder(body, event, booking)?;

        messages.push(
            Email::new(
                message_type,
                booking.email.clone(),
                subject.clone(),
                body,
                None,
            )
            .into_message(&email_account, email_gateway)?,
        );
    }

    email_gateway
        .send_messages(&email_account, messages)
        .await?;

    let booking_ids = bookings
        .into_iter()
        .map(|booking| booking.booking_id)
        .collect::<Vec<_>>();
    db::mark_as_payment_reminder_sent(pool, &booking_ids).await?;

    Ok(booking_ids.len())
}

pub(crate) async fn send_participation_confirmation(
    pool: &PgPool,
    event_id: EventId,
    email_gateway: &impl email::EmailGateway,
) -> Result<usize> {
    let mut event = db::get_event(pool, &event_id, true)
        .await?
        .ok_or_else(|| anyhow!("Error fetching event with id '{}'", event_id.get_ref()))?;
    let subscribers = event.subscribers.take().ok_or_else(|| {
        anyhow!(
            "Subscribers of event with id '{}' are missing",
            event_id.get_ref()
        )
    })?;

    let template = match event.event_type {
        EventType::Fitness => Ok(include_str!(
            "../../../templates/participation_confirmation_fitness.txt"
        )),
        EventType::Events => Err(anyhow!(
            "Participation confirmation is not supported for event type 'Events'."
        )),
    }?;

    let dates_len = event.dates.len();
    if dates_len < 1 {
        return Err(anyhow!(
            "Participation confirmation is not supported for an event without dates."
        ));
    }

    let fmt = "%d. %B %Y";
    let first_date = event
        .dates
        .first()
        .ok_or_else(|| anyhow!("Event with id '{}' has no first date", event_id))?
        .format_localized(fmt, Locale::de_DE)
        .to_string();
    let last_date = event
        .dates
        .last()
        .ok_or_else(|| anyhow!("Event with id '{}' has no last date", event_id))?
        .format_localized(fmt, Locale::de_DE)
        .to_string();
    let dates = format!("{dates_len} x {} Minuten", event.duration_in_minutes);

    let email_account = event.get_associated_email_account(email_gateway).await?;
    let subject = format!("{} Teilnahmebestätigung", event.subject_prefix());
    let mut messages = Vec::new();
    for subscriber in subscribers {
        if subscriber.enrolled {
            let price = subscriber.total_price(&event).to_euro();

            let bytes = export::create_participation_confirmation(
                subscriber.first_name.clone(),
                subscriber.last_name.clone(),
                event.name.clone(),
                first_date.clone(),
                last_date.clone(),
                price,
                dates.clone(),
            )
            .await?;

            let body = template::render_participation_confirmation(template, &event, &subscriber)?;

            let message = email_gateway
                .build_message(&email_account)?
                .to(subscriber.email.parse()?)
                .subject(subject.clone())
                .multipart(
                    MultiPart::mixed()
                        .singlepart(SinglePart::plain(body))
                        .singlepart(
                            Attachment::new(String::from("Teilnahmebestätigung.pdf"))
                                .body(bytes, ContentType::parse("application/pdf")?),
                        ),
                )?;

            messages.push(message)
        }
    }

    let count = messages.len();
    if count > 0 {
        email_gateway
            .send_messages(&email_account, messages)
            .await?;
    }

    Ok(count)
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use bigdecimal::BigDecimal;
    use chrono::{Duration, Utc};
    use pretty_assertions::assert_eq;
    use sqlx::PgPool;

    use crate::models::{EventType, LifecycleStatus, PartialEvent};
    use crate::test_utils::mock_email_gateway;

    use super::*;

    #[sqlx::test]
    async fn test_send_event_reminders_empty_db(pool: PgPool) -> Result<()> {
        let mock_sender = mock_email_gateway(vec![]).0;
        let count = send_event_reminders(&pool, &mock_sender).await?;
        assert_eq!(count, 0);
        Ok(())
    }

    #[sqlx::test]
    async fn test_send_participation_confirmation_events_type(pool: PgPool) -> Result<()> {
        let event = crate::db::write_event(
            &pool,
            PartialEvent {
                event_type: Some(EventType::Events),
                lifecycle_status: Some(LifecycleStatus::Running),
                name: Some("Events Type".to_string()),
                sort_index: Some(0),
                short_description: Some("Short".to_string()),
                description: Some("Desc".to_string()),
                image: Some("img.png".to_string()),
                light: Some(true),
                dates: Some(vec![Utc::now() + Duration::try_days(30).unwrap()]),
                duration_in_minutes: Some(60),
                max_subscribers: Some(10),
                max_waiting_list: Some(5),
                price_member: Some(BigDecimal::from(20)),
                price_non_member: Some(BigDecimal::from(25)),
                location: Some("Location".to_string()),
                booking_template: Some("Template".to_string()),
                payment_account: Some("DE1234".to_string()),
                external_operator: Some(false),
                ..Default::default()
            },
        )
        .await?;

        let mock_sender = mock_email_gateway(vec![]).0;
        let result = send_participation_confirmation(&pool, event.0.id, &mock_sender).await;
        assert!(result.is_err(), "Should fail for Events type");
        let err_msg = format!("{:?}", result.unwrap_err());
        assert!(err_msg.contains("not supported"), "Error was: {err_msg}");

        Ok(())
    }

    #[sqlx::test]
    async fn test_send_participation_confirmation_no_dates(pool: PgPool) -> Result<()> {
        let event = crate::db::write_event(
            &pool,
            PartialEvent {
                event_type: Some(EventType::Fitness),
                lifecycle_status: Some(LifecycleStatus::Running),
                name: Some("Fitness Event".to_string()),
                sort_index: Some(0),
                short_description: Some("Short".to_string()),
                description: Some("Desc".to_string()),
                image: Some("img.png".to_string()),
                light: Some(true),
                custom_date: Some("Sometime".to_string()),
                duration_in_minutes: Some(60),
                max_subscribers: Some(10),
                max_waiting_list: Some(5),
                price_member: Some(BigDecimal::from(20)),
                price_non_member: Some(BigDecimal::from(25)),
                location: Some("Location".to_string()),
                booking_template: Some("Template".to_string()),
                payment_account: Some("DE1234".to_string()),
                external_operator: Some(false),
                ..Default::default()
            },
        )
        .await?;

        let mock_sender = mock_email_gateway(vec![]).0;
        let result = send_participation_confirmation(&pool, event.0.id, &mock_sender).await;
        assert!(result.is_err());
        Ok(())
    }

    #[sqlx::test]
    async fn test_send_payment_reminders(pool: PgPool) -> Result<()> {
        let mock_sender = mock_email_gateway(vec![(
            crate::models::EmailType::Fitness,
            "test@example.com",
        )])
        .0;

        let result = send_payment_reminders(&pool, EventType::Fitness, &mock_sender).await;
        if let Err(e) = &result {
            eprintln!("Error: {:?}", e);
        }
        assert!(result.is_ok());
        assert_eq!(result?, 0);

        Ok(())
    }

    #[sqlx::test]
    async fn test_send_event_email_no_bookings(pool: PgPool) -> Result<()> {
        let event = crate::logic::events::create_test_event(&pool, LifecycleStatus::Published).await?;
        let mock_sender = mock_email_gateway(vec![(
            crate::models::EmailType::Fitness,
            "test@example.com",
        )])
        .0;

        let email = EventEmail {
            event_id: event.id,
            bookings: true,
            waiting_list: false,
            prebooking_event_id: None,
            subject: "Test Subject".to_string(),
            body: "Hello {{firstname}}".to_string(),
            attachments: None,
        };

        send_event_email(&pool, email, &mock_sender).await?;
        Ok(())
    }

    #[sqlx::test]
    async fn test_send_event_email_no_bookings_or_waiting_list_errors(pool: PgPool) -> Result<()> {
        let mock_sender = mock_email_gateway(vec![]).0;

        let email = EventEmail {
            event_id: 1.into(),
            bookings: false,
            waiting_list: false,
            prebooking_event_id: None,
            subject: "Test".to_string(),
            body: "Body".to_string(),
            attachments: None,
        };

        let result = send_event_email(&pool, email, &mock_sender).await;
        assert!(result.is_err(), "Should fail when neither bookings nor waiting_list selected");
        Ok(())
    }

    #[sqlx::test]
    async fn test_send_event_email_with_bookings(pool: PgPool) -> Result<()> {
        use crate::models::{EmailType, EventBooking};

        let event = crate::logic::events::create_test_event(&pool, LifecycleStatus::Published).await?;
        let (mock_sender, captured) = mock_email_gateway(vec![(
            EmailType::Fitness,
            "test@example.com",
        )]);

        let booking_data = EventBooking {
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

        crate::logic::events::booking(&pool, booking_data, &mock_sender).await;

        let email = EventEmail {
            event_id: event.id,
            bookings: true,
            waiting_list: false,
            prebooking_event_id: None,
            subject: "Event Update".to_string(),
            body: "Hello {{firstname}}, {{payment_details}}".to_string(),
            attachments: None,
        };

        send_event_email(&pool, email, &mock_sender).await?;

        let messages = captured.lock().unwrap();
        let email_messages: Vec<_> = messages
            .iter()
            .filter(|(_, msgs)| !msgs.is_empty())
            .flat_map(|(_, msgs)| msgs)
            .collect();
        assert!(!email_messages.is_empty(), "Should have sent at least one event email");

        Ok(())
    }
}
