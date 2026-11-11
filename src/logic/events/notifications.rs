use std::collections::HashMap;
use std::collections::hash_map::Entry;

use anyhow::{Result, anyhow, bail};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use chrono::{Duration, Locale, Utc};
use chrono_tz::Europe::Berlin;
use sqlx::PgPool;

use crate::db;
use crate::email;
use crate::logic::{export, template};
use crate::models::{Email, EmailAttachment, EventEmail, EventId, EventType, MessageType, ToEuro};

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

        let (body, html_body) = template::render_booking_generic(
            &data.body,
            &booking,
            &event,
            Some(payment_id),
            prebooking_link,
            None,
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
            .with_html(html_body)
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

        let (subject, body, html_template) = match event.event_type {
            EventType::Fitness => (
                format!("{} Info zum Kursstart", event.subject_prefix()),
                include_str!("../../../templates/event_reminder_fitness.txt"),
                "event_reminder_fitness",
            ),
            EventType::Events => (
                format!("{} Info zum Eventstart", event.subject_prefix()),
                include_str!("../../../templates/event_reminder_events.txt"),
                "event_reminder_events",
            ),
        };

        if let Some(subscribers) = &event.subscribers {
            for subscriber in subscribers.iter().filter(|s| s.enrolled) {
                let body = template::render_event_reminder(body, event, subscriber)?;
                let html_body =
                    template::render_event_reminder_html(html_template, event, subscriber)?;

                messages.push(
                    Email::new(
                        message_type,
                        subscriber.email.clone(),
                        subject.clone(),
                        body,
                        None,
                    )
                    .with_html(html_body)
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

pub(crate) async fn send_manager_event_reminders(
    pool: &PgPool,
    lead_days: i32,
    email_gateway: &impl email::EmailGateway,
) -> Result<usize> {
    if lead_days < 0 {
        bail!("Manager reminder lead days must not be negative");
    }

    let today = Utc::now().with_timezone(&Berlin).date_naive();
    let target_date = today
        .checked_add_signed(Duration::days(i64::from(lead_days)))
        .ok_or_else(|| anyhow!("Manager reminder target date is out of range"))?;
    let events = db::get_manager_reminder_events(pool, target_date, lead_days).await?;

    let mut sent_count = 0;
    for event in events {
        let recipients = event
            .manager_recipients
            .as_ref()
            .ok_or_else(|| anyhow!("Manager recipients missing for event {}", event.id))?;
        let subscribers = event
            .subscribers
            .as_ref()
            .ok_or_else(|| anyhow!("Subscribers missing for event {}", event.id))?;
        if recipients.is_empty() {
            continue;
        }

        let enrolled_count = subscribers
            .iter()
            .filter(|subscriber| subscriber.enrolled)
            .count();
        let waiting_count = subscribers
            .iter()
            .filter(|subscriber| !subscriber.enrolled)
            .count();
        let open_places = if event.max_subscribers < 0 {
            "unbegrenzt".to_string()
        } else {
            (i32::from(event.max_subscribers) - enrolled_count as i32)
                .max(0)
                .to_string()
        };
        let dates = event
            .dates
            .iter()
            .map(|date| {
                format!(
                    "- {}",
                    date.with_timezone(&Berlin)
                        .format_localized("%A, %-d. %B %Y um %H:%M Uhr", Locale::de_DE)
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        let mut body = format!(
            "Operative Erinnerung zum bevorstehenden Event\n\nVeranstaltung: {}\nTermine:\n{}\nOrt: {}\nAnmeldungen: {}\nWarteliste: {}\nFreie Plätze: {}",
            event.name, dates, event.location, enrolled_count, waiting_count, open_places
        );
        if let Some(cost_per_date) = &event.cost_per_date {
            // Keep this calculation aligned with SVE Tools' EventBookings.finacialResult.
            let income = subscribers
                .iter()
                .filter(|subscriber| subscriber.enrolled)
                .fold(bigdecimal::BigDecimal::from(0), |total, subscriber| {
                    total
                        + if subscriber.member {
                            event.price_member.clone()
                        } else {
                            event.price_non_member.clone()
                        }
                });
            let date_count = if event.custom_date.is_some() {
                1
            } else {
                event.dates.len()
            };
            let net_result =
                income - cost_per_date.clone() * bigdecimal::BigDecimal::from(date_count as i64);
            body.push_str(&format!(
                "\nVoraussichtliches Nettoergebnis: {}",
                net_result.to_euro()
            ));
        }

        let email_account = event.get_associated_email_account(email_gateway).await?;
        let message_type: MessageType = event.event_type.into();
        let subject = format!("Operative Erinnerung: {}", event.name);
        let messages = recipients
            .iter()
            .map(|recipient| {
                Email::new(
                    message_type,
                    recipient.clone(),
                    subject.clone(),
                    body.clone(),
                    None,
                )
                .into_message(&email_account, email_gateway)
            })
            .collect::<Result<Vec<_>>>()?;

        email_gateway
            .send_messages(&email_account, messages)
            .await?;
        db::mark_manager_reminder_sent(pool, &event.id, lead_days).await?;
        sent_count += 1;
    }

    Ok(sent_count)
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
    let (body, html_template) = match event_type {
        EventType::Fitness => (
            include_str!("../../../templates/payment_reminder_fitness.txt"),
            "payment_reminder_fitness",
        ),
        EventType::Events => (
            include_str!("../../../templates/payment_reminder_events.txt"),
            "payment_reminder_events",
        ),
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
        let html_body = template::render_payment_reminder_html(html_template, event, booking)?;

        messages.push(
            Email::new(
                message_type,
                booking.email.clone(),
                subject.clone(),
                body,
                None,
            )
            .with_html(html_body)
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
            let html_body = template::render_participation_confirmation_html(
                "participation_confirmation_fitness",
                &event,
                &subscriber,
            )?;

            let attachment = EmailAttachment {
                name: "Teilnahmebestätigung.pdf".to_string(),
                mime_type: "application/pdf".to_string(),
                data: STANDARD.encode(&bytes),
            };

            let message = Email::new(
                MessageType::Fitness,
                subscriber.email.clone(),
                subject.clone(),
                body,
                Some(vec![attachment]),
            )
            .with_html(html_body)
            .into_message(&email_account, email_gateway)?;

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
    use chrono::{DateTime, Duration, NaiveDate, Utc};
    use chrono_tz::Europe::Berlin;
    use pretty_assertions::assert_eq;
    use sqlx::PgPool;

    use crate::models::{EventType, LifecycleStatus, PartialEvent, PaymentMethod};
    use crate::test_utils::mock_email_gateway;

    use super::*;

    fn berlin_datetime(date: NaiveDate, hour: u32) -> DateTime<Utc> {
        date.and_hms_opt(hour, 0, 0)
            .unwrap()
            .and_local_timezone(Berlin)
            .single()
            .unwrap()
            .with_timezone(&Utc)
    }

    async fn create_manager_reminder_event(
        pool: &PgPool,
        name: &str,
        event_type: EventType,
        lifecycle_status: LifecycleStatus,
        dates: Vec<DateTime<Utc>>,
        custom_date: Option<&str>,
        manager_recipients: Vec<&str>,
        cost_per_date: Option<BigDecimal>,
    ) -> Result<crate::models::Event> {
        let (event, _) = crate::db::write_event(
            pool,
            PartialEvent {
                event_type: Some(event_type),
                lifecycle_status: Some(lifecycle_status),
                name: Some(name.to_string()),
                sort_index: Some(0),
                short_description: Some("Short".to_string()),
                description: Some("Description".to_string()),
                image: Some("image.png".to_string()),
                light: Some(true),
                dates: Some(dates),
                custom_date: custom_date.map(str::to_string),
                duration_in_minutes: Some(60),
                max_subscribers: Some(4),
                max_waiting_list: Some(3),
                price_member: Some(BigDecimal::from(20)),
                price_non_member: Some(BigDecimal::from(30)),
                cost_per_date,
                location: Some("Eutingen".to_string()),
                booking_template: Some("Template".to_string()),
                payment_account: Some("Account".to_string()),
                external_operator: Some(false),
                manager_recipients: Some(
                    manager_recipients.into_iter().map(str::to_string).collect(),
                ),
                ..Default::default()
            },
        )
        .await?;
        Ok(event)
    }

    async fn add_manager_reminder_booking(
        pool: &PgPool,
        event_id: &crate::models::EventId,
        email: &str,
        member: bool,
        enrolled: bool,
        payment_id: &str,
    ) -> Result<()> {
        let subscriber_id: i32 = sqlx::query_scalar!(
            "INSERT INTO event_subscribers (first_name, last_name, street, city, email, member) \
             VALUES ('Private', 'Participant', 'Street', 'Eutingen', $1, $2) RETURNING id",
            email,
            member
        )
        .fetch_one(pool)
        .await?;

        sqlx::query!(
            "INSERT INTO event_bookings (event_id, subscriber_id, enrolled, pre_booking, payment_id) \
             VALUES ($1, $2, $3, false, $4)",
            event_id.get_ref(),
            subscriber_id,
            enrolled,
            payment_id
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    #[sqlx::test]
    async fn test_manager_event_reminders_send_operational_snapshot_and_are_idempotent(
        pool: PgPool,
    ) -> Result<()> {
        use crate::models::EmailType;

        let today = Utc::now().with_timezone(&Berlin).date_naive();
        let target_date = today.checked_add_days(chrono::Days::new(7)).unwrap();
        let event = create_manager_reminder_event(
            &pool,
            "Sommerfest",
            EventType::Events,
            LifecycleStatus::Review,
            vec![
                berlin_datetime(target_date, 0),
                berlin_datetime(target_date, 18),
            ],
            None,
            vec!["manager@example.com", "assistant@example.com"],
            Some(BigDecimal::from(10)),
        )
        .await?;
        create_manager_reminder_event(
            &pool,
            "Fitness event",
            EventType::Fitness,
            LifecycleStatus::Published,
            vec![berlin_datetime(target_date, 12)],
            None,
            vec!["fitness-manager@example.com"],
            None,
        )
        .await?;
        create_manager_reminder_event(
            &pool,
            "Draft event",
            EventType::Events,
            LifecycleStatus::Draft,
            vec![berlin_datetime(target_date, 12)],
            None,
            vec!["draft-manager@example.com"],
            None,
        )
        .await?;
        create_manager_reminder_event(
            &pool,
            "Wrong date event",
            EventType::Events,
            LifecycleStatus::Published,
            vec![berlin_datetime(target_date + Duration::days(2), 12)],
            None,
            vec!["wrong-date-manager@example.com"],
            None,
        )
        .await?;
        create_manager_reminder_event(
            &pool,
            "Later matching date event",
            EventType::Events,
            LifecycleStatus::Published,
            vec![
                berlin_datetime(target_date - Duration::days(1), 12),
                berlin_datetime(target_date, 12),
            ],
            None,
            vec!["later-date-manager@example.com"],
            None,
        )
        .await?;
        create_manager_reminder_event(
            &pool,
            "Free-form date event",
            EventType::Events,
            LifecycleStatus::Published,
            vec![],
            Some("Some Saturday"),
            vec!["custom-date-manager@example.com"],
            None,
        )
        .await?;
        create_manager_reminder_event(
            &pool,
            "No managers event",
            EventType::Events,
            LifecycleStatus::Published,
            vec![berlin_datetime(target_date, 12)],
            None,
            vec![],
            None,
        )
        .await?;
        add_manager_reminder_booking(
            &pool,
            &event.id,
            "participant@example.com",
            true,
            true,
            "manager-test-enrolled",
        )
        .await?;
        add_manager_reminder_booking(
            &pool,
            &event.id,
            "waiting@example.com",
            false,
            false,
            "manager-test-waiting",
        )
        .await?;
        add_manager_reminder_booking(
            &pool,
            &event.id,
            "second-participant@example.com",
            false,
            true,
            "manager-test-enrolled-second",
        )
        .await?;

        let (email_gateway, captured) =
            mock_email_gateway(vec![(EmailType::Events, "events@example.com")]);
        assert_eq!(
            send_manager_event_reminders(&pool, 7, &email_gateway).await?,
            1
        );

        let messages = captured
            .lock()
            .unwrap()
            .iter()
            .flat_map(|(_, messages)| messages.iter())
            .map(|message| String::from_utf8(message.formatted()).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(messages.len(), 2);
        for (message, recipient) in messages
            .iter()
            .zip(["manager@example.com", "assistant@example.com"])
        {
            assert!(message.contains(recipient));
            assert!(message.contains("Sommerfest"));
            assert!(message.contains("Eutingen"));
            assert!(message.contains("Anmeldungen: 2"));
            assert!(message.contains("Warteliste: 1"));
            assert!(message.contains("Freie Pl=C3=A4tze: 2"));
            assert!(message.contains("30,00"));
            assert!(!message.contains("Private"));
            assert!(!message.contains("Participant"));
            assert!(!message.contains("Hallo"));
            assert!(!message.contains("Viele Grüße"));
        }

        assert_eq!(
            send_manager_event_reminders(&pool, 7, &email_gateway).await?,
            0
        );
        assert_eq!(captured.lock().unwrap().len(), 1);

        let next_date = target_date + Duration::days(1);
        crate::db::write_event(
            &pool,
            PartialEvent {
                id: Some(event.id),
                dates: Some(vec![berlin_datetime(next_date, 12)]),
                ..Default::default()
            },
        )
        .await?;
        assert_eq!(
            send_manager_event_reminders(&pool, 8, &email_gateway).await?,
            1
        );
        assert_eq!(captured.lock().unwrap().len(), 2);
        Ok(())
    }

    #[sqlx::test]
    async fn test_manager_event_reminders_send_without_enrollments_or_cost(
        pool: PgPool,
    ) -> Result<()> {
        use crate::models::EmailType;

        let today = Utc::now().with_timezone(&Berlin).date_naive();
        let target_date = today.checked_add_days(chrono::Days::new(7)).unwrap();
        create_manager_reminder_event(
            &pool,
            "Leeres Event",
            EventType::Events,
            LifecycleStatus::Published,
            vec![berlin_datetime(target_date, 23)],
            None,
            vec!["manager@example.com"],
            None,
        )
        .await?;

        let (email_gateway, captured) =
            mock_email_gateway(vec![(EmailType::Events, "events@example.com")]);
        assert_eq!(
            send_manager_event_reminders(&pool, 7, &email_gateway).await?,
            1
        );
        let captured = captured.lock().unwrap();
        let messages = captured
            .iter()
            .flat_map(|(_, messages)| messages.iter())
            .map(|message| String::from_utf8(message.formatted()).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("Anmeldungen: 0"));
        assert!(messages[0].contains("Warteliste: 0"));
        assert!(!messages[0].contains("Nettoergebnis"));
        assert!(messages[0].contains("manager@example.com"));
        Ok(())
    }

    #[sqlx::test]
    async fn test_failed_manager_reminder_delivery_can_be_retried(pool: PgPool) -> Result<()> {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };

        use crate::email::MockEmailGateway;
        use crate::models::{EmailAccount, EmailType};

        let today = Utc::now().with_timezone(&Berlin).date_naive();
        let target_date = today.checked_add_days(chrono::Days::new(7)).unwrap();
        create_manager_reminder_event(
            &pool,
            "Retry event",
            EventType::Events,
            LifecycleStatus::Running,
            vec![berlin_datetime(target_date, 12)],
            None,
            vec!["manager@example.com"],
            None,
        )
        .await?;

        let account = EmailAccount::new_for_test(EmailType::Events, "events@example.com");
        let mut email_gateway = MockEmailGateway::new();
        email_gateway.expect_account_by_type().returning(move |_| {
            let account = account.clone();
            Box::pin(async move { Ok(account) })
        });
        email_gateway.expect_build_message().returning(|account| {
            Ok(lettre::Message::builder()
                .from(account.address.parse()?)
                .date_now())
        });
        let attempts = Arc::new(AtomicUsize::new(0));
        let send_attempts = Arc::clone(&attempts);
        email_gateway
            .expect_send_messages()
            .times(2)
            .returning(move |_, _| {
                let attempt = send_attempts.fetch_add(1, Ordering::SeqCst);
                Box::pin(async move {
                    if attempt == 0 {
                        Err(anyhow::anyhow!("simulated delivery failure"))
                    } else {
                        Ok(())
                    }
                })
            });

        assert!(
            send_manager_event_reminders(&pool, 7, &email_gateway)
                .await
                .is_err()
        );
        assert_eq!(
            send_manager_event_reminders(&pool, 7, &email_gateway).await?,
            1
        );
        assert_eq!(
            send_manager_event_reminders(&pool, 7, &email_gateway).await?,
            0
        );
        assert_eq!(attempts.load(Ordering::SeqCst), 2);
        Ok(())
    }

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
        let event =
            crate::logic::events::create_test_event(&pool, LifecycleStatus::Published).await?;
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
        assert!(
            result.is_err(),
            "Should fail when neither bookings nor waiting_list selected"
        );
        Ok(())
    }

    #[sqlx::test]
    async fn test_send_event_email_with_bookings(pool: PgPool) -> Result<()> {
        use crate::models::{EmailType, EventBooking};

        let event =
            crate::logic::events::create_test_event(&pool, LifecycleStatus::Published).await?;
        let (mock_sender, captured) =
            mock_email_gateway(vec![(EmailType::Fitness, "test@example.com")]);

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
        assert!(
            !email_messages.is_empty(),
            "Should have sent at least one event email"
        );

        Ok(())
    }

    #[sqlx::test]
    async fn test_send_event_reminders_sends_multipart_alternative(pool: PgPool) -> Result<()> {
        use crate::db;
        use crate::models::{EmailType, EventBooking};

        let (mock_sender, captured) =
            mock_email_gateway(vec![(EmailType::Fitness, "fitness@test.com")]);

        let event = db::write_event(
            &pool,
            PartialEvent {
                event_type: Some(EventType::Fitness),
                lifecycle_status: Some(LifecycleStatus::Published),
                name: Some("Reminder Event".to_string()),
                sort_index: Some(0),
                short_description: Some("Short".to_string()),
                description: Some("Desc".to_string()),
                image: Some("img.png".to_string()),
                light: Some(true),
                dates: Some(vec![Utc::now() + Duration::try_days(3).unwrap()]),
                duration_in_minutes: Some(60),
                max_subscribers: Some(10),
                max_waiting_list: Some(5),
                price_member: Some(BigDecimal::from(20)),
                price_non_member: Some(BigDecimal::from(25)),
                location: Some("Test Location".to_string()),
                booking_template: Some("Template".to_string()),
                payment_account: Some("DE1234".to_string()),
                external_operator: Some(false),
                ..Default::default()
            },
        )
        .await?;

        let booking_data = EventBooking {
            event_id: event.0.id,
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

        captured.lock().unwrap().clear();

        let count = send_event_reminders(&pool, &mock_sender).await?;
        assert_eq!(count, 1);

        let sent = captured.lock().unwrap();
        let messages: Vec<_> = sent.iter().flat_map(|(_, msgs)| msgs).collect();
        assert_eq!(messages.len(), 1);

        let formatted = String::from_utf8(messages[0].formatted()).unwrap();
        assert!(
            formatted.contains("Content-Type: multipart/alternative"),
            "Should be multipart/alternative"
        );
        assert!(
            formatted.contains("text/plain"),
            "Should contain text/plain"
        );
        assert!(formatted.contains("text/html"), "Should contain text/html");
        assert!(formatted.contains("Hallo Max"), "Should contain greeting");

        Ok(())
    }

    #[sqlx::test]
    async fn test_send_payment_reminders_sends_multipart_alternative(pool: PgPool) -> Result<()> {
        use crate::db;
        use crate::models::EmailType;
        use chrono::{DateTime, Utc};

        let (mock_sender, captured) =
            mock_email_gateway(vec![(EmailType::Events, "events@test.com")]);

        let event = db::write_event(
            &pool,
            PartialEvent {
                event_type: Some(EventType::Events),
                lifecycle_status: Some(LifecycleStatus::Published),
                name: Some("Payment Reminder Event".to_string()),
                sort_index: Some(0),
                short_description: Some("Short".to_string()),
                description: Some("Desc".to_string()),
                image: Some("img.png".to_string()),
                light: Some(true),
                dates: Some(vec![Utc::now() - Duration::try_days(60).unwrap()]),
                duration_in_minutes: Some(60),
                max_subscribers: Some(10),
                max_waiting_list: Some(5),
                price_member: Some(BigDecimal::from(20)),
                price_non_member: Some(BigDecimal::from(25)),
                location: Some("Test Location".to_string()),
                booking_template: Some("Template".to_string()),
                payment_account: Some("DE1234".to_string()),
                payment_method: Some(PaymentMethod::BankTransfer),
                external_operator: Some(false),
                ..Default::default()
            },
        )
        .await?;
        let event_id = event.0.id.into_inner();

        sqlx::query!(
            r#"INSERT INTO event_subscribers (first_name, last_name, street, city, email, phone, member)
            VALUES ($1, $2, $3, $4, $5, $6, $7)"#,
            "Max",
            "Mustermann",
            "Teststr 1",
            "Teststadt",
            "max@test.com",
            None::<String>,
            true,
        )
        .execute(&pool)
        .await?;

        let subscriber_row = sqlx::query!(
            r#"SELECT id FROM event_subscribers WHERE email = $1"#,
            "max@test.com"
        )
        .fetch_one(&pool)
        .await?;

        sqlx::query!(
            r#"INSERT INTO event_bookings (event_id, subscriber_id, enrolled, pre_booking, canceled, payment_id, payment_confirmed_at, created)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)"#,
            event_id,
            subscriber_row.id,
            true,
            false,
            None::<DateTime<Utc>>,
            "pay_001",
            None::<DateTime<Utc>>,
            Utc::now() - Duration::try_days(30).unwrap(),
        )
        .execute(&pool)
        .await?;

        let count = send_payment_reminders(&pool, EventType::Events, &mock_sender).await?;
        assert_eq!(count, 1);

        let sent = captured.lock().unwrap();
        let messages: Vec<_> = sent.iter().flat_map(|(_, msgs)| msgs).collect();
        assert_eq!(messages.len(), 1);

        let formatted = String::from_utf8(messages[0].formatted()).unwrap();
        assert!(
            formatted.contains("Content-Type: multipart/alternative"),
            "Should be multipart/alternative, got: {}",
            formatted
        );
        assert!(
            formatted.contains("text/plain"),
            "Should contain text/plain"
        );
        assert!(formatted.contains("text/html"), "Should contain text/html");
        assert!(formatted.contains("Hallo Max"), "Should contain greeting");

        Ok(())
    }

    #[sqlx::test]
    async fn test_send_participation_confirmation_sends_nested_mixed(pool: PgPool) -> Result<()> {
        use crate::db;
        use crate::models::EmailType;

        let (mock_sender, captured) =
            mock_email_gateway(vec![(EmailType::Fitness, "fitness@test.com")]);

        let event = db::write_event(
            &pool,
            PartialEvent {
                event_type: Some(EventType::Fitness),
                lifecycle_status: Some(LifecycleStatus::Published),
                name: Some("Participation Event".to_string()),
                sort_index: Some(0),
                short_description: Some("Short".to_string()),
                description: Some("Desc".to_string()),
                image: Some("img.png".to_string()),
                light: Some(true),
                dates: Some(vec![
                    Utc::now() + Duration::try_days(30).unwrap(),
                    Utc::now() + Duration::try_days(37).unwrap(),
                ]),
                duration_in_minutes: Some(60),
                max_subscribers: Some(10),
                max_waiting_list: Some(5),
                price_member: Some(BigDecimal::from(20)),
                price_non_member: Some(BigDecimal::from(25)),
                location: Some("Test Location".to_string()),
                booking_template: Some("Template".to_string()),
                payment_account: Some("DE1234".to_string()),
                external_operator: Some(false),
                ..Default::default()
            },
        )
        .await?;

        let event_id = event.0.id.into_inner();

        sqlx::query!(
            r#"INSERT INTO event_subscribers (first_name, last_name, street, city, email, phone, member)
            VALUES ($1, $2, $3, $4, $5, $6, $7)"#,
            "Max",
            "Mustermann",
            "Teststr 1",
            "Teststadt",
            "max@test.com",
            None::<String>,
            true,
        )
        .execute(&pool)
        .await?;

        let subscriber_row = sqlx::query!(
            r#"SELECT id FROM event_subscribers WHERE email = $1"#,
            "max@test.com"
        )
        .fetch_one(&pool)
        .await?;

        sqlx::query!(
            r#"INSERT INTO event_bookings (event_id, subscriber_id, enrolled, pre_booking, canceled, payment_id, payment_confirmed_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7)"#,
            event_id,
            subscriber_row.id,
            true,
            false,
            None::<chrono::DateTime<Utc>>,
            "pay_001",
            None::<chrono::DateTime<Utc>>,
        )
        .execute(&pool)
        .await?;

        let count = send_participation_confirmation(&pool, event.0.id, &mock_sender).await?;
        assert_eq!(count, 1);

        let sent = captured.lock().unwrap();
        let messages: Vec<_> = sent.iter().flat_map(|(_, msgs)| msgs).collect();
        assert_eq!(messages.len(), 1);

        let formatted = String::from_utf8(messages[0].formatted()).unwrap();
        assert!(formatted.contains("multipart/mixed"), "no mixed");
        assert!(
            formatted.contains("multipart/alternative"),
            "no alternative"
        );
        assert!(formatted.contains("text/plain"), "no plain");
        assert!(formatted.contains("text/html"), "no html");
        assert!(formatted.contains("application/pdf"), "no pdf");
        assert!(formatted.contains("Hallo Max"), "no greeting");

        Ok(())
    }
}
