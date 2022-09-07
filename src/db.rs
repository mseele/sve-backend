use crate::models::{
    Event, EventBooking, EventCounter, EventId, EventType, LifecycleStatus, NewsSubscription,
    NewsTopic, PartialEvent, VerifyPaymentBookingRecord,
};
use anyhow::{anyhow, bail, Result};
use chrono::{DateTime, Utc};
use itertools::Itertools;
use sqlx::{
    postgres::PgPoolOptions, query, query_as, query_builder::Separated, query_scalar, FromRow,
    PgConnection, PgPool, Postgres, QueryBuilder, Row,
};
use std::collections::{HashMap, HashSet};

const DATABASE_URL: &str = include_str!("../secrets/database_url.env");

pub async fn init_pool() -> Result<PgPool> {
    let pool = PgPoolOptions::new().connect(DATABASE_URL).await?;
    Ok(pool)
}

pub async fn get_events(
    pool: &PgPool,
    sort: bool,
    lifecycle_status: Option<Vec<LifecycleStatus>>,
) -> Result<Vec<Event>> {
    let mut conn = pool.acquire().await?;

    let mut query_builder: QueryBuilder<Postgres> = QueryBuilder::new(
        r#"
SELECT
    e.id,
    e.created,
    e.closed,
    e.event_type,
    e.lifecycle_status,
    e.name,
    e.sort_index,
    e.short_description,
    e.description,
    e.image,
    e.light,
    e.custom_date,
    e.duration_in_minutes,
    e.max_subscribers,
    e.max_waiting_list,
    e.cost_member,
    e.cost_non_member,
    e.location,
    e.booking_template,
    e.waiting_template,
    e.alt_booking_button_text,
    e.alt_email_address,
    e.external_operator,
    vev.subscribers,
    vev.waiting_list
FROM
    events e,
    v_event_counters vev
WHERE
    e.id = vev.id"#,
    );
    if let Some(lifecycle_status) = &lifecycle_status {
        query_builder.push(
            r#"
 AND e.lifecycle_status IN("#,
        );
        let mut separated = query_builder.separated(", ");
        for value in lifecycle_status.iter() {
            separated.push_bind(value);
        }
        query_builder.push(
            r#")
"#,
        );
    }

    query_builder.push(
        r#"
ORDER BY
    e.sort_index,
    e.created"#,
    );

    let mut result = Vec::new();
    for row in query_builder.build().fetch_all(&mut *conn).await? {
        let subscribers: i64 = row.try_get("subscribers")?;
        let waiting_list: i64 = row.try_get("waiting_list")?;

        result.push((
            Event::new(
                row.try_get("id")?,
                row.try_get("created")?,
                row.try_get("closed")?,
                row.try_get("event_type")?,
                row.try_get("lifecycle_status")?,
                row.try_get("name")?,
                row.try_get("sort_index")?,
                row.try_get("short_description")?,
                row.try_get("description")?,
                row.try_get("image")?,
                row.try_get("light")?,
                Vec::new(),
                row.try_get("custom_date")?,
                row.try_get("duration_in_minutes")?,
                row.try_get("max_subscribers")?,
                row.try_get("max_waiting_list")?,
                row.try_get("cost_member")?,
                row.try_get("cost_non_member")?,
                row.try_get("location")?,
                row.try_get("booking_template")?,
                row.try_get("waiting_template")?,
                row.try_get("alt_booking_button_text")?,
                row.try_get("alt_email_address")?,
                row.try_get("external_operator")?,
            ),
            // try_into is needed to convert the i64 into a i16
            EventCounter::new(
                row.try_get("id")?,
                row.try_get("max_subscribers")?,
                row.try_get("max_waiting_list")?,
                subscribers.try_into()?,
                waiting_list.try_into()?,
            ),
        ));
    }

    let mut iter = result.into_iter();
    let mut events: Vec<Event>;
    if sort {
        iter = iter.sorted_by(|(a, ca), (b, cb)| {
            let is_a_booked_up = ca.is_booked_up();
            let is_b_booked_up = cb.is_booked_up();
            if is_a_booked_up == is_b_booked_up {
                return a.sort_index.cmp(&b.sort_index);
            }
            return is_a_booked_up.cmp(&is_b_booked_up);
        })
    }
    events = iter.map(|(event, _)| event).collect();

    fetch_dates(&mut conn, &mut events).await?;

    Ok(events)
}

pub async fn get_event(pool: &PgPool, id: EventId) -> Result<Option<Event>> {
    let mut conn = pool.acquire().await?;
    Ok(fetch_event(&mut conn, &id).await?)
}

async fn fetch_event(conn: &mut PgConnection, id: &EventId) -> Result<Option<Event>> {
    let mut event: Option<Event> = query!(
        r#"
SELECT
    e.id,
    e.created,
    e.closed,
    e.event_type AS "event_type: EventType",
    e.lifecycle_status AS "lifecycle_status: LifecycleStatus",
    e.name,
    e.sort_index,
    e.short_description,
    e.description,
    e.image,
    e.light,
    e.custom_date,
    e.duration_in_minutes,
    e.max_subscribers,
    e.max_waiting_list,
    e.cost_member,
    e.cost_non_member,
    e.location,
    e.booking_template,
    e.waiting_template,
    e.alt_booking_button_text,
    e.alt_email_address,
    e.external_operator
FROM
    events e
WHERE
    e.id = $1"#,
        id.get_ref()
    )
    .map(|row| {
        Event::new(
            row.id,
            row.created,
            row.closed,
            row.event_type,
            row.lifecycle_status,
            row.name,
            row.sort_index,
            row.short_description,
            row.description,
            row.image,
            row.light,
            Vec::new(),
            row.custom_date,
            row.duration_in_minutes,
            row.max_subscribers,
            row.max_waiting_list,
            row.cost_member,
            row.cost_non_member,
            row.location,
            row.booking_template,
            row.waiting_template,
            row.alt_booking_button_text,
            row.alt_email_address,
            row.external_operator,
        )
    })
    .fetch_optional(&mut *conn)
    .await?;

    if let Some(value) = event {
        event = fetch_dates(&mut *conn, &mut vec![value]).await?.pop();
    }

    Ok(event)
}

async fn fetch_dates<'a>(
    conn: &mut PgConnection,
    events: &'a mut Vec<Event>,
) -> Result<&'a mut Vec<Event>> {
    if events.is_empty() {
        return Ok(events);
    }

    let mut query_builder: QueryBuilder<Postgres> = QueryBuilder::new(
        r#"
SELECT
    e.event_id,
    e.date
FROM
    event_dates e
WHERE
    e.event_id IN ("#,
    );
    let mut separated = query_builder.separated(", ");
    for event in events.iter() {
        separated.push_bind(event.id.get_ref());
    }
    separated.push_unseparated(
        r#")
ORDER BY
    e.event_id,
    e.date"#,
    );

    let mut result = HashMap::new();
    for row in query_builder.build().fetch_all(conn).await? {
        let id: i32 = row.try_get("event_id")?;
        let date: DateTime<Utc> = row.try_get("date")?;
        result.entry(id).or_insert_with(|| Vec::new()).push(date);
    }

    for event in events.iter_mut() {
        if let Some(dates) = result.remove(&event.id.get_ref()) {
            event.dates = dates;
        }
    }

    Ok(events)
}

pub async fn write_event(pool: &PgPool, partial_event: PartialEvent) -> Result<Event> {
    match partial_event.id {
        Some(id) => update_event(pool, &id, partial_event).await,
        None => save_new_event(pool, partial_event).await,
    }
}

async fn update_event(pool: &PgPool, id: &EventId, partial_event: PartialEvent) -> Result<Event> {
    let mut tx = pool.begin().await?;

    let mut query_builder: QueryBuilder<Postgres> = QueryBuilder::new("UPDATE events SET ");
    let mut update_is_needed = false;
    let mut separated = query_builder.separated(", ");
    update_is_needed |= push_bind(&mut separated, "CLOSED", partial_event.closed);
    update_is_needed |= push_bind(&mut separated, "EVENT_TYPE", partial_event.event_type);
    update_is_needed |= push_bind(
        &mut separated,
        "LIFECYCLE_STATUS",
        partial_event.lifecycle_status,
    );
    update_is_needed |= push_bind(&mut separated, "NAME", partial_event.name);
    update_is_needed |= push_bind(&mut separated, "SORT_INDEX", partial_event.sort_index);
    update_is_needed |= push_bind(
        &mut separated,
        "SHORT_DESCRIPTION",
        partial_event.short_description,
    );
    update_is_needed |= push_bind(&mut separated, "DESCRIPTION", partial_event.description);
    update_is_needed |= push_bind(&mut separated, "IMAGE", partial_event.image);
    update_is_needed |= push_bind(&mut separated, "LIGHT", partial_event.light);
    update_is_needed |= push_bind(&mut separated, "CUSTOM_DATE", partial_event.custom_date);
    update_is_needed |= push_bind(
        &mut separated,
        "DURATION_IN_MINUTES",
        partial_event.duration_in_minutes,
    );
    update_is_needed |= push_bind(
        &mut separated,
        "MAX_SUBSCRIBERS",
        partial_event.max_subscribers,
    );
    update_is_needed |= push_bind(
        &mut separated,
        "MAX_WAITING_LIST",
        partial_event.max_waiting_list,
    );
    update_is_needed |= push_bind(&mut separated, "COST_MEMBER", partial_event.cost_member);
    update_is_needed |= push_bind(
        &mut separated,
        "COST_NON_MEMBER",
        partial_event.cost_non_member,
    );
    update_is_needed |= push_bind(&mut separated, "LOCATION", partial_event.location);
    update_is_needed |= push_bind(
        &mut separated,
        "BOOKING_TEMPLATE",
        partial_event.booking_template,
    );
    update_is_needed |= push_bind(
        &mut separated,
        "WAITING_TEMPLATE",
        partial_event.waiting_template,
    );
    update_is_needed |= push_bind(
        &mut separated,
        "ALT_BOOKING_BUTTON_TEXT",
        partial_event.alt_booking_button_text,
    );
    update_is_needed |= push_bind(
        &mut separated,
        "ALT_EMAIL_ADDRESS",
        partial_event.alt_email_address,
    );
    update_is_needed |= push_bind(
        &mut separated,
        "EXTERNAL_OPERATOR",
        partial_event.external_operator,
    );

    if update_is_needed {
        query_builder.push("WHERE id = ");
        query_builder.push_bind(id.get_ref());

        query_builder.build().execute(&mut tx).await?;
    }

    if let Some(dates) = partial_event.dates {
        delete_event_dates(&mut tx, id).await?;
        insert_event_dates(&mut tx, id, dates).await?;
    }

    let event = fetch_event(&mut tx, id)
        .await?
        .ok_or_else(|| anyhow!("Error fetching event with id '{}'", id))?;

    tx.commit().await?;

    Ok(event)
}

fn push_bind<'gb, 'args, T>(
    separated: &mut Separated<'gb, 'args, Postgres, &str>,
    key: &str,
    value: Option<T>,
) -> bool
where
    T: 'args + sqlx::Encode<'args, Postgres> + Send + sqlx::Type<Postgres>,
{
    match value {
        Some(v) => {
            separated
                .push(key)
                .push_unseparated(" = ")
                .push_bind_unseparated(v);
            true
        }
        None => false,
    }
}

async fn save_new_event(pool: &PgPool, partial_event: PartialEvent) -> Result<Event> {
    let closed = partial_event.closed;
    let event_type = partial_event
        .event_type
        .ok_or_else(|| anyhow!("Attribute 'event_type' is missing"))?;
    let lifecycle_status = partial_event
        .lifecycle_status
        .ok_or_else(|| anyhow!("Attribute 'lifecycle_status' is missing"))?;
    let name = partial_event
        .name
        .ok_or_else(|| anyhow!("Attribute 'name' is missing"))?;
    let sort_index = partial_event
        .sort_index
        .ok_or_else(|| anyhow!("Attribute 'sort_index' is missing"))?;
    let short_description = partial_event
        .short_description
        .ok_or_else(|| anyhow!("Attribute 'short_description' is missing"))?;
    let description = partial_event
        .description
        .ok_or_else(|| anyhow!("Attribute 'description' is missing"))?;
    let image = partial_event
        .image
        .ok_or_else(|| anyhow!("Attribute 'image' is missing"))?;
    let light = partial_event
        .light
        .ok_or_else(|| anyhow!("Attribute 'light' is missing"))?;
    let dates = match partial_event.custom_date {
        Some(_) => Vec::new(),
        None => partial_event
            .dates
            .ok_or_else(|| anyhow!("Attribute 'dates' is missing"))?,
    };
    let custom_date = partial_event.custom_date;
    let duration_in_minutes = partial_event
        .duration_in_minutes
        .ok_or_else(|| anyhow!("Attribute 'duration_in_minutes' is missing"))?;
    let max_subscribers = partial_event
        .max_subscribers
        .ok_or_else(|| anyhow!("Attribute 'max_subscribers' is missing"))?;
    let max_waiting_list = partial_event
        .max_waiting_list
        .ok_or_else(|| anyhow!("Attribute 'max_waiting_list' is missing"))?;
    let cost_member = partial_event
        .cost_member
        .ok_or_else(|| anyhow!("Attribute 'cost_member' is missing"))?;
    let cost_non_member = partial_event
        .cost_non_member
        .ok_or_else(|| anyhow!("Attribute 'cost_non_member' is missing"))?;
    let location = partial_event
        .location
        .ok_or_else(|| anyhow!("Attribute 'location' is missing"))?;
    let booking_template = partial_event
        .booking_template
        .ok_or_else(|| anyhow!("Attribute 'booking_template' is missing"))?;
    let waiting_template = partial_event
        .waiting_template
        .ok_or_else(|| anyhow!("Attribute 'waiting_template' is missing"))?;
    let alt_booking_button_text = partial_event.alt_booking_button_text;
    let alt_email_address = partial_event.alt_email_address;
    let external_operator = partial_event
        .external_operator
        .ok_or_else(|| anyhow!("Attribute 'external_operator' is missing"))?;

    let mut tx = pool.begin().await?;

    let mut new_event: Event = query!(
        r#"
INSERT INTO events (closed, event_type, lifecycle_status, name, sort_index, short_description, description, image, light, custom_date, duration_in_minutes, max_subscribers, max_waiting_list, cost_member, cost_non_member, location, booking_template, waiting_template, alt_booking_button_text, alt_email_address, external_operator)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21)
RETURNING id, created, closed, event_type AS "event_type: EventType", lifecycle_status AS "lifecycle_status: LifecycleStatus", name, sort_index, short_description, description, image, light, custom_date, duration_in_minutes, max_subscribers, max_waiting_list, cost_member, cost_non_member, location, booking_template, waiting_template, alt_booking_button_text, alt_email_address, external_operator"#,
        closed,
        event_type as EventType,
        lifecycle_status as LifecycleStatus,
        name,
        sort_index,
        short_description,
        description,
        image,
        light,
        custom_date,
        duration_in_minutes,
        max_subscribers,
        max_waiting_list,
        cost_member,
        cost_non_member,
        location,
        booking_template,
        waiting_template,
        alt_booking_button_text,
        alt_email_address,
        external_operator
    )
    .map(|row| {
        Event::new(
            row.id.into(),
            row.created,
            row.closed,
            row.event_type,
            row.lifecycle_status,
            row.name,
            row.sort_index,
            row.short_description,
            row.description,
            row.image,
            row.light,
            Vec::new(),
            row.custom_date,
            row.duration_in_minutes,
            row.max_subscribers,
            row.max_waiting_list,
            row.cost_member,
            row.cost_non_member,
            row.location,
            row.booking_template,
            row.waiting_template,
            row.alt_booking_button_text,
            row.alt_email_address,
            row.external_operator,
        )
    })
    .fetch_one(&mut tx)
    .await?;

    delete_event_dates(&mut tx, &new_event.id).await?;
    new_event.dates = insert_event_dates(&mut tx, &new_event.id, dates).await?;

    tx.commit().await?;

    Ok(new_event)
}

async fn delete_event_dates(conn: &mut PgConnection, event_id: &EventId) -> Result<()> {
    query!(
        r#"DELETE FROM event_dates WHERE event_id = $1"#,
        event_id.get_ref()
    )
    .execute(conn)
    .await?;

    Ok(())
}

async fn insert_event_dates(
    conn: &mut PgConnection,
    event_id: &EventId,
    dates: Vec<DateTime<Utc>>,
) -> Result<Vec<DateTime<Utc>>> {
    let ids = vec![event_id.into_inner(); dates.len()];
    query!(
        r#"INSERT INTO event_dates (event_id, date) SELECT * FROM UNNEST ($1::int4[], $2::timestamptz[])"#,
        &ids,
        &dates
    )
    .execute(conn)
    .await?;

    Ok(dates)
}

pub async fn delete_event(pool: &PgPool, id: EventId) -> Result<()> {
    let mut tx = pool.begin().await?;

    let lifecycle_status: Option<LifecycleStatus> = query!(
        r#"SELECT e.lifecycle_status AS "lifecycle_status: LifecycleStatus" FROM events e WHERE e.id = $1"#,
        id.get_ref()
    )
    .map(|row| row.lifecycle_status)
    .fetch_optional(&mut tx)
    .await?;

    let lifecycle_status = lifecycle_status
        .ok_or_else(|| anyhow!("Event with id {} has not been found in the database.", id))?;

    if !matches!(lifecycle_status, LifecycleStatus::Draft) {
        bail!(
            "Cannot delete event {} with lifecycle status {:?}",
            id,
            lifecycle_status
        )
    }

    delete_event_dates(&mut tx, &id).await?;

    query!(r#"DELETE FROM events e WHERE e.id = $1"#, id.get_ref())
        .execute(&mut tx)
        .await?;

    tx.commit().await?;

    Ok(())
}

pub async fn get_bookings_to_verify_payment(
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
        THEN e.cost_member
        ESLE e.cost_non_member
    END as cost,
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
                row.get("cost"),
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

pub async fn mark_as_payed(pool: &PgPool, verified_payments: &HashMap<i32, String>) -> Result<()> {
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
        .execute(&mut tx)
        .await?;
    }

    tx.commit().await?;

    Ok(())
}

pub async fn get_event_counters(
    pool: &PgPool,
    lifecycle_status: LifecycleStatus,
) -> Result<Vec<EventCounter>> {
    let mut conn = pool.acquire().await?;

    Ok(fetch_event_counters(&mut conn, lifecycle_status).await?)
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
        // unwrap is needed because view columns are always "nullable"
        // try_into is needed to convert the i64 into a i16
        // both unwraps can never fail
        EventCounter::new(
            row.id.into(),
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

pub enum BookingResult {
    Booked(Event, Vec<EventCounter>, String),
    WaitingList(Event, Vec<EventCounter>, String),
    DuplicateBooking,
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

pub async fn book_event(pool: &PgPool, booking: &EventBooking) -> Result<BookingResult> {
    let mut tx = pool.begin().await?;

    let result = match calc_enroll_status(&mut tx, &booking.event_id).await? {
        Some(enrolled) => process_booking(&mut tx, booking, enrolled, false).await?,
        None => BookingResult::BookedOut,
    };

    tx.commit().await?;

    Ok(result)
}

pub async fn pre_book_event(
    pool: &PgPool,
    event_id: EventId,
    subscriber_id: i32,
) -> Result<(BookingResult, Option<EventBooking>)> {
    let mut tx = pool.begin().await?;

    let result = match calc_enroll_status(&mut tx, &event_id).await? {
        Some(enrolled) => {
            let result = insert_booking(
                &mut tx,
                &event_id,
                &EventSubscriberId::Existing(subscriber_id),
                enrolled,
                true,
                &None,
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
                event_id.get_ref()
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
                )
            })
            .fetch_one(&mut tx)
            .await?;

            (result, Some(booking))
        }
        None => (BookingResult::BookedOut, None),
    };

    tx.commit().await?;

    Ok(result)
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
        // unwrap is needed because view columns are always "nullable"
        // try_into is needed to convert the i64 into a i16
        // both unwraps can never fail
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
"#,
            event_id.get_ref(),
            id
        )
        .map(|row| row.count)
        .fetch_one(&mut *conn)
        .await?;

        if let Some(v) = count {
            if v > 0 {
                return Ok(BookingResult::DuplicateBooking);
            }
        }
    }

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
(event_id, enrolled, pre_booking, subscriber_id, comment, payment_id)
VALUES($1, $2, $3, $4, $5, $6)"#,
        event_id.get_ref(),
        enrolled,
        pre_booking,
        subscriber_id.get_id(),
        *comments,
        payment_id
    )
    .execute(&mut *conn)
    .await?;

    let event = fetch_event(&mut *conn, &event_id)
        .await?
        .ok_or_else(|| anyhow!("Found no event with id '{}'", event_id))?;
    let event_counters = fetch_event_counters(&mut *conn, event.lifecycle_status).await?;

    if let true = enrolled {
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
    if let Some(id) = get_event_subscriber_id(conn, &booking).await? {
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
        booking.member
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

pub async fn get_subscriptions(pool: &PgPool) -> Result<Vec<NewsSubscription>> {
    let subscriptions =
        query!(r#"SELECT s.email, s.general, s.events, s.fitness FROM news_subscribers s"#)
            .map(|row| {
                let mut types = Vec::new();
                if row.general {
                    types.push(NewsTopic::General);
                }
                if row.events {
                    types.push(NewsTopic::Events);
                }
                if row.fitness {
                    types.push(NewsTopic::Fitness);
                }
                return NewsSubscription::new(row.email, types);
            })
            .fetch_all(pool)
            .await?;

    Ok(subscriptions)
}

pub async fn subscribe(pool: &PgPool, subscription: NewsSubscription) -> Result<NewsSubscription> {
    let mut tx = pool.begin().await?;
    let current_subscription = get_current_subscription(&mut tx, &subscription.email).await?;

    let general = subscription.topics.contains(&NewsTopic::General);
    let events = subscription.topics.contains(&NewsTopic::Events);
    let fitness = subscription.topics.contains(&NewsTopic::Fitness);

    if let Some(current_subscriptions) = current_subscription {
        update_subscription(
            &mut tx,
            current_subscriptions.id,
            current_subscriptions.general || general,
            current_subscriptions.events || events,
            current_subscriptions.fitness || fitness,
        )
        .await?;
    } else {
        query!(
            r#"INSERT INTO news_subscribers (email, general, events, fitness) VALUES($1, $2, $3, $4)"#,
            &subscription.email,
            general,
            events,
            fitness
        ).execute(&mut tx)
        .await?;
    }

    tx.commit().await?;

    Ok(subscription)
}

pub async fn unsubscribe(pool: &PgPool, subscription: &NewsSubscription) -> Result<()> {
    let mut tx = pool.begin().await?;

    let current_subscription = get_current_subscription(&mut tx, &subscription.email).await?;

    if let Some(current_subscription) = current_subscription {
        let general =
            current_subscription.general && !subscription.topics.contains(&NewsTopic::General);
        let events =
            current_subscription.events && !subscription.topics.contains(&NewsTopic::Events);
        let fitness =
            current_subscription.fitness && !subscription.topics.contains(&NewsTopic::Fitness);
        if general || events || fitness {
            update_subscription(&mut tx, current_subscription.id, general, events, fitness).await?;
        } else {
            query!(
                r#"DELETE FROM news_subscribers WHERE id = $1"#,
                current_subscription.id
            )
            .execute(&mut tx)
            .await?;
        }
    }

    tx.commit().await?;

    Ok(())
}

#[derive(FromRow)]
struct CurrentSubscription {
    id: i32,
    general: bool,
    events: bool,
    fitness: bool,
}

async fn get_current_subscription(
    conn: &mut PgConnection,
    email: &str,
) -> Result<Option<CurrentSubscription>> {
    let current_subscription: Option<CurrentSubscription> = query_as!(
        CurrentSubscription,
        r#"SELECT s.id, s.general, s.events, s.fitness FROM news_subscribers s WHERE s.email = $1"#,
        email
    )
    .fetch_optional(conn)
    .await?;

    Ok(current_subscription)
}

async fn update_subscription(
    conn: &mut PgConnection,
    id: i32,
    general: bool,
    events: bool,
    fitness: bool,
) -> Result<()> {
    query!(
        r#"UPDATE news_subscribers SET general = $2, events = $3, fitness = $4 WHERE id = $1"#,
        id,
        general,
        events,
        fitness
    )
    .execute(conn)
    .await?;

    Ok(())
}