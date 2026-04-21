use std::collections::{HashMap, HashSet};

use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use sqlx::{PgConnection, PgPool, Postgres, QueryBuilder, Row, query, query_scalar};

use super::events::fetch_event;
use crate::models::{
    Event, EventBooking, EventCounter, EventCustomField, EventCustomFieldType, EventId, EventType,
    LifecycleStatus, UnpaidEventBooking, VerifyPaymentBookingRecord,
};

pub(crate) async fn get_bookings_to_verify_payment(
    pool: &PgPool,
    payment_ids: HashSet<&String>,
) -> Result<Vec<VerifyPaymentBookingRecord>> {
    let mut query_builder: QueryBuilder<Postgres> = QueryBuilder::new(
        r#"
SELECT
    b.id,
    e.name AS event_name,
    CONCAT (s.first_name, ' ', s.last_name) AS full_name,
    CASE WHEN s.member IS TRUE
        THEN e.price_member
        ELSE e.price_non_member
    END as price,
    b.payment_id,
    b.canceled,
    b.enrolled,
    b.payed
FROM
    events e,
    event_bookings b,
    event_subscribers s
WHERE
    e.id = b.event_id
    AND b.subscriber_id = s.id
    AND b.payment_id IN("#,
    );
    let mut separated = query_builder.separated(", ");
    for payment_id in payment_ids {
        separated.push_bind(payment_id);
    }
    separated.push_unseparated(
        r#")
ORDER BY
    b.created"#,
    );
    let result = query_builder
        .build()
        .map(|row| {
            VerifyPaymentBookingRecord::new(
                row.get("id"),
                row.get("event_name"),
                row.get("full_name"),
                row.get("price"),
                row.get("payment_id"),
                row.get("canceled"),
                row.get("enrolled"),
                row.get("payed"),
            )
        })
        .fetch_all(pool)
        .await?;

    Ok(result)
}

pub(crate) async fn get_event_bookings_without_payment(
    pool: &PgPool,
    event_type: EventType,
) -> Result<
    Vec<(
        UnpaidEventBooking,
        DateTime<Utc>,
        Option<DateTime<Utc>>,
        String,
    )>,
> {
    let result = query!(
        r#"
SELECT
    e.id AS event_id,
    e.name AS event_name,
    ed.date as first_event_date,
    e.booking_template as event_template,
    b.id,
    b.created,
    s.first_name,
    s.last_name,
    s.email,
    CASE WHEN s.member IS TRUE
        THEN e.price_member
        ELSE e.price_non_member
    END as price,
    b.payment_id,
    b.payment_reminder_sent
FROM
    events e
    LEFT JOIN (
        SELECT
            ied.event_id,
            MIN(ied.date) as date
        FROM
            event_dates ied
        GROUP BY
            ied.event_id) ed ON
        e.id = ed.event_id,
    event_bookings b,
    event_subscribers s
WHERE
    e.event_type = $1
    AND e.id = b.event_id
    AND b.subscriber_id = s.id
    AND b.enrolled IS TRUE
    AND b.canceled IS NULL
    AND b.payed IS NULL
	AND e.lifecycle_status IN('Review', 'Published', 'Running')
ORDER BY
    b.payment_reminder_sent,
    e.name,
    b.created"#,
        event_type as EventType
    )
    .map(|row| {
        (
            UnpaidEventBooking::new(
                row.event_id.into(),
                row.event_name,
                row.id,
                row.created,
                row.first_name,
                row.last_name,
                row.email,
                row.price.unwrap(),
                row.payment_id,
                None,
                row.payment_reminder_sent,
            ),
            row.created,
            row.first_event_date,
            row.event_template,
        )
    })
    .fetch_all(pool)
    .await?;

    Ok(result)
}

pub(crate) async fn mark_as_payed(
    pool: &PgPool,
    verified_payments: &HashMap<i32, String>,
) -> Result<()> {
    let mut tx = pool.begin().await?;

    // TODO: improve by using batch update
    for (booking_id, iban) in verified_payments {
        query!(
            r#"
UPDATE
    event_bookings
SET
    payed = NOW(),
    iban = $1
WHERE
    id = $2"#,
            iban,
            booking_id,
        )
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    Ok(())
}

pub(crate) async fn update_payment(
    pool: &PgPool,
    booking_id: i32,
    update_payment: bool,
) -> Result<()> {
    let mut tx = pool.begin().await?;

    match update_payment {
        true => {
            query!(
                r#"UPDATE event_bookings SET payed = NOW() WHERE id = $1"#,
                booking_id,
            )
            .execute(&mut *tx)
            .await?;
        }
        false => {
            query!(
                r#"UPDATE event_bookings SET payed = NULL WHERE id = $1"#,
                booking_id,
            )
            .execute(&mut *tx)
            .await?;
        }
    }

    tx.commit().await?;

    Ok(())
}

pub(crate) async fn get_all_custom_fields(
    pool: &sqlx::Pool<Postgres>,
) -> Result<Vec<EventCustomField>> {
    let result = query!(
        r#"
SELECT
    id,
    name,
    type AS "cf_type: EventCustomFieldType",
    min_value,
    max_value
FROM
    event_custom_fields
"#
    )
    .map(|row| EventCustomField::new(row.id, row.name, row.cf_type, row.min_value, row.max_value))
    .fetch_all(pool)
    .await?;

    Ok(result)
}

pub(crate) async fn get_event_counters(
    pool: &PgPool,
    lifecycle_status: LifecycleStatus,
) -> Result<Vec<EventCounter>> {
    let mut conn = pool.acquire().await?;

    fetch_event_counters(&mut conn, lifecycle_status).await
}

pub(crate) async fn get_bookings(
    pool: &PgPool,
    event_id: &EventId,
    enrolled: Option<bool>,
) -> Result<Vec<(EventBooking, i32, String)>> {
    let mut conn = pool.acquire().await?;

    let mut query_builder: QueryBuilder<Postgres> = QueryBuilder::new(
        r#"
SELECT
    v.event_id,
    v.subscriber_id,
    v.first_name,
    v.last_name,
    v.street,
    v.city,
    v.email,
    v.phone,
    v.member,
    v.payment_id,
    v.custom_value_1,
    v.custom_value_2,
    v.custom_value_3,
    v.custom_value_4
FROM
    v_event_bookings v
WHERE
    v.event_id = "#,
    );
    query_builder.push_bind(event_id.get_ref()).push(
        r#"
    AND v.canceled IS NULL"#,
    );

    if let Some(enrolled) = enrolled {
        query_builder
            .push(
                r#"
    AND v.enrolled = "#,
            )
            .push_bind(enrolled);
    }

    query_builder.push(
        r#"
ORDER BY
    v.created"#,
    );

    let mut result = Vec::new();
    for row in query_builder.build().fetch_all(&mut *conn).await? {
        result.push((
            EventBooking::new(
                row.try_get("event_id")?,
                row.try_get("first_name")?,
                row.try_get("last_name")?,
                row.try_get("street")?,
                row.try_get("city")?,
                row.try_get("email")?,
                row.try_get("phone")?,
                row.try_get("member")?,
                None,
                None,
                vec![
                    row.try_get::<Option<String>, _>("custom_value_1")?,
                    row.try_get::<Option<String>, _>("custom_value_2")?,
                    row.try_get::<Option<String>, _>("custom_value_3")?,
                    row.try_get::<Option<String>, _>("custom_value_4")?,
                ]
                .into_iter()
                .flatten()
                .collect(),
            ),
            row.try_get("subscriber_id")?,
            row.try_get("payment_id")?,
        ));
    }

    Ok(result)
}

async fn fetch_event_counters(
    conn: &mut PgConnection,
    lifecycle_status: LifecycleStatus,
) -> Result<Vec<EventCounter>> {
    let event_counters = query!(
        r#"
SELECT
    e.id,
    v.max_subscribers,
    v.max_waiting_list,
    v.subscribers,
    v.waiting_list
FROM
    events e,
    v_event_counters v
WHERE
    e.id = v.id
    AND e.lifecycle_status = $1"#,
        lifecycle_status as LifecycleStatus
    )
    .map(|row| {
        EventCounter::new(
            row.id,
            row.max_subscribers.unwrap(),
            row.max_waiting_list.unwrap(),
            row.subscribers.unwrap().try_into().unwrap(),
            row.waiting_list.unwrap().try_into().unwrap(),
        )
    })
    .fetch_all(conn)
    .await?;

    Ok(event_counters)
}

pub(crate) enum BookingResult {
    Booked(Event, Vec<EventCounter>, String),
    WaitingList(Event, Vec<EventCounter>, String),
    DuplicateBooking,
    NotBookable,
    BookedOut,
}

enum EventSubscriberId {
    New(i32),
    Existing(i32),
}

impl EventSubscriberId {
    fn get_id(&self) -> &i32 {
        match self {
            EventSubscriberId::New(id) => id,
            EventSubscriberId::Existing(id) => id,
        }
    }
}

pub(crate) async fn book_event(pool: &PgPool, booking: &EventBooking) -> Result<BookingResult> {
    let mut tx = pool.begin().await?;

    if !is_event_bookable(&mut tx, &booking.event_id).await? {
        return Ok(BookingResult::NotBookable);
    }

    let result = match calc_enroll_status(&mut tx, &booking.event_id).await? {
        Some(enrolled) => process_booking(&mut tx, booking, enrolled, false).await?,
        None => BookingResult::BookedOut,
    };

    tx.commit().await?;

    Ok(result)
}

pub(crate) async fn pre_book_event(
    pool: &PgPool,
    event_id: EventId,
    subscriber_id: i32,
) -> Result<(BookingResult, Option<EventBooking>)> {
    let mut tx = pool.begin().await?;

    if !is_event_bookable(&mut tx, &event_id).await? {
        return Ok((BookingResult::NotBookable, None));
    }

    let result = match calc_enroll_status(&mut tx, &event_id).await? {
        Some(enrolled) => {
            let result = insert_booking(
                &mut tx,
                &event_id,
                &EventSubscriberId::Existing(subscriber_id),
                enrolled,
                true,
                &None,
                &[],
            )
            .await?;

            let booking = query!(
                r#"
SELECT
    e.first_name,
    e.last_name,
    e.street,
    e.city,
    e.email,
    e.phone,
    e.member
FROM
    event_subscribers e
WHERE
    e.id = $1"#,
                subscriber_id
            )
            .map(|row| {
                EventBooking::new(
                    event_id.into_inner(),
                    row.first_name,
                    row.last_name,
                    row.street,
                    row.city,
                    row.email,
                    row.phone,
                    Some(row.member),
                    None,
                    None,
                    Vec::new(),
                )
            })
            .fetch_one(&mut *tx)
            .await?;

            (result, Some(booking))
        }
        None => (BookingResult::BookedOut, None),
    };

    tx.commit().await?;

    Ok(result)
}

async fn is_event_bookable(conn: &mut PgConnection, event_id: &EventId) -> Result<bool> {
    let lifecycle = query!(
        r#"
SELECT
    e.lifecycle_status AS "lifecycle_status: LifecycleStatus"
FROM
    events e
WHERE
    e.id = $1"#,
        event_id.get_ref()
    )
    .map(|row| row.lifecycle_status)
    .fetch_one(conn)
    .await?;

    Ok(lifecycle.is_bookable())
}

async fn calc_enroll_status(conn: &mut PgConnection, event_id: &EventId) -> Result<Option<bool>> {
    let result = query!(
        r#"
SELECT
    v.id,
    v.max_subscribers,
    v.max_waiting_list,
    v.subscribers,
    v.waiting_list
FROM
    v_event_counters v
WHERE
    v.id = $1"#,
        event_id.get_ref(),
    )
    .map(|row| {
        EventCounter::new(
            row.id.unwrap(),
            row.max_subscribers.unwrap(),
            row.max_waiting_list.unwrap(),
            row.subscribers.unwrap().try_into().unwrap(),
            row.waiting_list.unwrap().try_into().unwrap(),
        )
    })
    .fetch_optional(conn)
    .await?;

    let event_counter = result.ok_or_else(|| anyhow!("Found no event with id '{}'", event_id))?;

    let enroll_status = if event_counter.max_subscribers == -1
        || event_counter.subscribers < event_counter.max_subscribers
    {
        Some(true)
    } else if event_counter.waiting_list < event_counter.max_waiting_list {
        Some(false)
    } else {
        None
    };

    Ok(enroll_status)
}

async fn process_booking(
    conn: &mut PgConnection,
    booking: &EventBooking,
    enrolled: bool,
    pre_booking: bool,
) -> Result<BookingResult> {
    let subscriber_id = insert_event_subscriber(conn, booking).await?;
    let result = insert_booking(
        conn,
        &booking.event_id,
        &subscriber_id,
        enrolled,
        pre_booking,
        &booking.comments,
        &booking.custom_values,
    )
    .await?;

    Ok(result)
}

async fn insert_booking(
    conn: &mut PgConnection,
    event_id: &EventId,
    subscriber_id: &EventSubscriberId,
    enrolled: bool,
    pre_booking: bool,
    comments: &Option<String>,
    custom_values: &[String],
) -> Result<BookingResult> {
    // check for duplicate booking
    if let EventSubscriberId::Existing(id) = subscriber_id {
        let count = query!(
            r#"
SELECT
	COUNT(1)
FROM
	event_bookings e
WHERE
	e.event_id = $1
	AND e.subscriber_id = $2
    AND e.canceled IS NULL
"#,
            event_id.get_ref(),
            id
        )
        .map(|row| row.count)
        .fetch_one(&mut *conn)
        .await?;

        if let Some(v) = count
            && v > 0
        {
            return Ok(BookingResult::DuplicateBooking);
        }
    }

    // trim comment
    let comment = comments
        .as_ref()
        .map(|comment| comment.trim())
        .filter(|comment| !comment.is_empty());

    // generate payment id
    let payment_id: Option<i64> = query_scalar!("SELECT nextval('payment_id')")
        .fetch_one(&mut *conn)
        .await?;
    let year = Utc::now().format("%y");
    let payment_id = format!("{}-{}", year, payment_id.unwrap());

    // insert booking
    query!(
        r#"
INSERT INTO public.event_bookings
(event_id, enrolled, pre_booking, subscriber_id, comment, payment_id, custom_value_1, custom_value_2, custom_value_3, custom_value_4)
VALUES($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)"#,
        event_id.get_ref(),
        enrolled,
        pre_booking,
        subscriber_id.get_id(),
        comment,
        payment_id,
        custom_values.first(),
        custom_values.get(1),
        custom_values.get(2),
        custom_values.get(3),
    )
    .execute(&mut *conn)
    .await?;

    let event = fetch_event(&mut *conn, event_id, false)
        .await?
        .ok_or_else(|| anyhow!("Found no event with id '{}'", event_id))?;
    let event_counters = fetch_event_counters(&mut *conn, event.lifecycle_status).await?;

    if enrolled {
        Ok(BookingResult::Booked(event, event_counters, payment_id))
    } else {
        Ok(BookingResult::WaitingList(
            event,
            event_counters,
            payment_id,
        ))
    }
}

async fn insert_event_subscriber(
    conn: &mut PgConnection,
    booking: &EventBooking,
) -> Result<EventSubscriberId> {
    if let Some(id) = get_event_subscriber_id(conn, booking).await? {
        return Ok(EventSubscriberId::Existing(id));
    }

    let id = query!(
        r#"
INSERT INTO event_subscribers (first_name, last_name, street, city, email, phone, member)
VALUES($1, $2, $3, $4, $5, $6, $7)
RETURNING id"#,
        booking.first_name,
        booking.last_name,
        booking.street,
        booking.city,
        booking.email,
        booking.phone,
        booking.member.or_else(|| Some(false))
    )
    .map(|row| row.id)
    .fetch_one(conn)
    .await?;

    Ok(EventSubscriberId::New(id))
}

async fn get_event_subscriber_id(
    conn: &mut PgConnection,
    booking: &EventBooking,
) -> Result<Option<i32>> {
    let mut query_builder: QueryBuilder<Postgres> =
        QueryBuilder::new("SELECT e.id FROM event_subscribers e WHERE ");

    let mut separated = query_builder.separated(" AND ");

    separated
        .push("e.first_name = ")
        .push_bind_unseparated(booking.first_name.clone());

    separated
        .push("e.last_name = ")
        .push_bind_unseparated(booking.last_name.clone());

    separated
        .push("e.street = ")
        .push_bind_unseparated(booking.street.clone());

    separated
        .push("e.city = ")
        .push_bind_unseparated(booking.city.clone());

    separated
        .push("e.email = ")
        .push_bind_unseparated(booking.email.clone());

    separated.push("e.phone ");
    match &booking.phone {
        Some(phone) => separated
            .push_unseparated(" = ")
            .push_bind_unseparated(phone.clone()),
        None => separated.push_unseparated(" IS NULL"),
    };

    separated.push("e.member ");
    match booking.member {
        Some(member) => separated
            .push_unseparated(" = ")
            .push_bind_unseparated(member),
        None => separated.push_unseparated(" IS NULL"),
    };

    let id = query_builder
        .build()
        .map(|row| row.get("id"))
        .fetch_optional(conn)
        .await?;

    Ok(id)
}

pub(crate) async fn cancel_event_booking(
    pool: &PgPool,
    booking_id: i32,
) -> Result<(Event, EventBooking, Option<(EventBooking, String)>)> {
    let mut tx = pool.begin().await?;

    // cancel booking
    query!(
        r#"UPDATE event_bookings SET canceled = NOW() WHERE id = $1"#,
        booking_id,
    )
    .execute(&mut *tx)
    .await?;

    // fetch the canceled booking data (include enrolled flag so we know
    // whether the canceled booking was an enrolled attendee or a waiting-list entry)
    let (canceled_booking, canceled_enrolled) = query!(
        r#"
SELECT
    v.event_id,
    v.first_name,
    v.last_name,
    v.street,
    v.city,
    v.email,
    v.phone,
    v.member,
    v.enrolled
FROM
    v_event_bookings v
WHERE
    v.id = $1"#,
        booking_id
    )
    .map(|row| {
        (
            EventBooking::new(
                row.event_id.unwrap(),
                row.first_name.unwrap(),
                row.last_name.unwrap(),
                row.street.unwrap(),
                row.city.unwrap(),
                row.email.unwrap(),
                row.phone,
                row.member,
                None,
                None,
                Vec::new(),
            ),
            row.enrolled.unwrap_or(false),
        )
    })
    .fetch_one(&mut *tx)
    .await?;

    let event_id = canceled_booking.event_id;

    // Only promote a waiting-list entry if the canceled booking was an enrolled attendee.
    // If the canceled booking itself was from the waiting list (enrolled == false),
    // we must not create a new attendee.
    let first_waiting_list_booking;
    if canceled_enrolled {
        // fetch the first waiting list entrance
        let waiting_list_result: Option<(i32, EventBooking, String)> = query!(
            r#"
SELECT
    v.id,
    v.first_name,
    v.last_name,
    v.street,
    v.city,
    v.email,
    v.phone,
    v.member,
    v.payment_id
FROM
    v_event_bookings v
WHERE
    v.event_id = $1
    AND v.canceled IS NULL
    AND v.enrolled IS FALSE
ORDER BY
    v.created"#,
            event_id.get_ref()
        )
        .map(|row| {
            (
                row.id.unwrap(),
                EventBooking::new(
                    event_id.into_inner(),
                    row.first_name.unwrap(),
                    row.last_name.unwrap(),
                    row.street.unwrap(),
                    row.city.unwrap(),
                    row.email.unwrap(),
                    row.phone,
                    row.member,
                    None,
                    None,
                    Vec::new(),
                ),
                row.payment_id.unwrap(),
            )
        })
        .fetch_optional(&mut *tx)
        .await?;

        // extract and switch enrolled status for waiting list booking - if available
        if let Some((booking_id, booking, payment_id)) = waiting_list_result {
            query!(
                r#"UPDATE event_bookings SET enrolled = true WHERE id = $1"#,
                booking_id,
            )
            .execute(&mut *tx)
            .await?;

            first_waiting_list_booking = Some((booking, payment_id));
        } else {
            first_waiting_list_booking = None;
        }
    } else {
        // canceled booking was on the waiting list -> do not promote anyone
        first_waiting_list_booking = None;
    }

    let event = fetch_event(&mut tx, &event_id, false)
        .await?
        .ok_or_else(|| anyhow!("Error fetching event with id '{}'", event_id))?;

    tx.commit().await?;

    Ok((event, canceled_booking, first_waiting_list_booking))
}

/// mark the given bookings that the payment reminder email has been sent
/// (to avoid duplicate sending of reminder emails)
pub(crate) async fn mark_as_payment_reminder_sent(
    pool: &PgPool,
    booking_ids: &[i32],
) -> Result<()> {
    if booking_ids.is_empty() {
        return Ok(());
    }

    let mut query_builder: QueryBuilder<Postgres> = QueryBuilder::new(
        r#"UPDATE
        event_bookings
    SET
        payment_reminder_sent = NOW()
    WHERE
        id IN ("#,
    );
    let mut separated = query_builder.separated(", ");
    for id in booking_ids.iter() {
        separated.push_bind(id);
    }
    separated.push_unseparated(")");
    query_builder.build().execute(pool).await?;

    Ok(())
}

#[cfg(test)]
mod db_integration_tests {
    use super::*;
    use crate::models::{EventType, LifecycleStatus, PartialEvent};
    use anyhow::Result;
    use chrono::{DateTime, Utc};
    use std::collections::HashMap;

    #[sqlx::test]
    async fn test_get_and_cancel_booking(pool: PgPool) -> Result<()> {
        let partial = PartialEvent {
            event_type: Some(EventType::Fitness),
            lifecycle_status: Some(LifecycleStatus::Published),
            name: Some("Fitness Class".to_string()),
            sort_index: Some(1),
            short_description: Some("Get fit".to_string()),
            description: Some("Full description".to_string()),
            image: Some("test.png".to_string()),
            light: Some(false),
            duration_in_minutes: Some(60),
            max_subscribers: Some(10),
            max_waiting_list: Some(5),
            price_member: Some("15.00".parse().unwrap()),
            price_non_member: Some("20.00".parse().unwrap()),
            location: Some("Studio".to_string()),
            booking_template: Some("Template".to_string()),
            payment_account: Some("Account".to_string()),
            external_operator: Some(false),
            dates: Some(vec![Utc::now()]),
            ..Default::default()
        };

        let (event, _) = crate::db::events::write_event(&pool, partial).await?;
        let event_id = event.id;

        query!(
            r#"INSERT INTO event_subscribers (first_name, last_name, street, city, email, phone, member) 
            VALUES ($1, $2, $3, $4, $5, $6, $7)"#,
            "John",
            "Doe",
            "Main St",
            "Vienna",
            "john@example.com",
            None::<String>,
            true
        )
        .execute(&pool)
        .await?;

        let subscriber_row =
            query!(r#"SELECT id FROM event_subscribers WHERE email = 'john@example.com'"#,)
                .fetch_one(&pool)
                .await?;
        let subscriber_id = subscriber_row.id;

        query!(
            r#"INSERT INTO event_bookings (event_id, subscriber_id, enrolled, pre_booking, canceled, payment_id, payed) 
            VALUES ($1, $2, $3, $4, $5, $6, $7)"#,
            event_id.get_ref(),
            subscriber_id,
            true,
            false,
            None::<DateTime<Utc>>,
            "pay_123",
            None::<DateTime<Utc>>,
        )
        .execute(&pool)
        .await?;

        let bookings = get_bookings(&pool, &event_id, None).await?;
        assert_eq!(bookings.len(), 1);
        let booking = &bookings[0].0;
        assert_eq!(booking.email, "john@example.com");
        let booking_id = bookings[0].1;

        let (canceled_event, canceled_booking, waiting_promotion) =
            cancel_event_booking(&pool, booking_id).await?;
        assert_eq!(canceled_event.id, event_id);
        assert_eq!(canceled_booking.email, "john@example.com");
        assert!(waiting_promotion.is_none());

        let bookings_after = get_bookings(&pool, &event_id, None).await?;
        assert_eq!(bookings_after.len(), 0);

        Ok(())
    }

    #[sqlx::test]
    async fn test_mark_as_payed_and_update_payment(pool: PgPool) -> Result<()> {
        let partial = PartialEvent {
            event_type: Some(EventType::Events),
            lifecycle_status: Some(LifecycleStatus::Published),
            name: Some("Paid Event".to_string()),
            sort_index: Some(1),
            short_description: Some("Pay here".to_string()),
            description: Some("Full description".to_string()),
            image: Some("test.png".to_string()),
            light: Some(false),
            duration_in_minutes: Some(90),
            max_subscribers: Some(20),
            max_waiting_list: Some(5),
            price_member: Some("50.00".parse().unwrap()),
            price_non_member: Some("60.00".parse().unwrap()),
            location: Some("Hall".to_string()),
            booking_template: Some("Template".to_string()),
            payment_account: Some("Account".to_string()),
            external_operator: Some(false),
            dates: Some(vec![Utc::now()]),
            ..Default::default()
        };

        let (event, _) = crate::db::events::write_event(&pool, partial).await?;
        let event_id = event.id;

        query!(
            r#"INSERT INTO event_subscribers (first_name, last_name, street, city, email, phone, member) 
            VALUES ($1, $2, $3, $4, $5, $6, $7)"#,
            "Jane",
            "Smith",
            "Second St",
            "Vienna",
            "jane@example.com",
            None::<String>,
            false
        )
        .execute(&pool)
        .await?;

        let subscriber_row =
            query!(r#"SELECT id FROM event_subscribers WHERE email = 'jane@example.com'"#,)
                .fetch_one(&pool)
                .await?;
        let subscriber_id = subscriber_row.id;

        query!(
            r#"INSERT INTO event_bookings (event_id, subscriber_id, enrolled, pre_booking, canceled, payment_id, payed) 
            VALUES ($1, $2, $3, $4, $5, $6, $7)"#,
            event_id.get_ref(),
            subscriber_id,
            true,
            false,
            None::<DateTime<Utc>>,
            "pay_456",
            None::<DateTime<Utc>>,
        )
        .execute(&pool)
        .await?;

        let booking_row = query!(r#"SELECT id FROM event_bookings WHERE payment_id = 'pay_456'"#,)
            .fetch_one(&pool)
            .await?;
        let booking_id = booking_row.id;

        update_payment(&pool, booking_id, true).await?;

        let booking_row = query!(
            r#"SELECT payed FROM event_bookings WHERE id = $1"#,
            booking_id
        )
        .fetch_one(&pool)
        .await?;
        assert!(booking_row.payed.is_some());

        let mut verified_payments = HashMap::new();
        verified_payments.insert(booking_id, "AT611904300234573200".to_string());
        mark_as_payed(&pool, &verified_payments).await?;

        let booking_row = query!(
            r#"SELECT payed, iban FROM event_bookings WHERE id = $1"#,
            booking_id
        )
        .fetch_one(&pool)
        .await?;
        assert!(booking_row.payed.is_some());
        assert_eq!(booking_row.iban.as_deref(), Some("AT611904300234573200"));

        Ok(())
    }
}
