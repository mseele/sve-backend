use crate::models::{
    Event, EventCounter, EventCustomField, EventCustomFieldType, EventId, EventSubscription,
    EventType, LifecycleStatus, PartialEvent,
};
use anyhow::{Result, anyhow, bail};
use chrono::{DateTime, Utc};
use itertools::Itertools;
use sqlx::{
    PgConnection, PgPool, Postgres, QueryBuilder, Row, postgres::PgRow, query,
    query_builder::Separated,
};
use std::collections::HashMap;

pub(crate) async fn get_events(
    pool: &PgPool,
    sort: bool,
    lifecycle_status: Option<Vec<LifecycleStatus>>,
    subscribers: bool,
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
    e.price_member,
    e.price_non_member,
    e.cost_per_date,
    e.location,
    e.booking_template,
    e.payment_account,
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
    } else {
        query_builder.push(
            r#"
 AND e.lifecycle_status != "#,
        );
        query_builder.push_bind(LifecycleStatus::Archived);
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
            map_event(&row)?,
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
            is_a_booked_up.cmp(&is_b_booked_up)
        })
    }
    events = iter.map(|(event, _)| event).collect();

    if !events.is_empty() {
        insert_event_dates(&mut conn, &mut events).await?;
        insert_event_custom_fields(&mut conn, &mut events).await?;
        if subscribers {
            insert_event_subscribers(&mut conn, &mut events).await?;
        }
    }

    Ok(events)
}

pub(crate) async fn get_event(
    pool: &PgPool,
    id: &EventId,
    subscribers: bool,
) -> Result<Option<Event>> {
    let mut conn = pool.acquire().await?;
    fetch_event(&mut conn, id, subscribers).await
}

/// Fetch a single event by the given event id.
/// Subscribers will be attached to the event if `subscribers` is true.
pub(crate) async fn fetch_event(
    conn: &mut PgConnection,
    id: &EventId,
    subscribers: bool,
) -> Result<Option<Event>> {
    Ok(fetch_events(conn, vec![*id], subscribers).await?.pop())
}

/// Fetch a list of events by the given event id's.
/// Subscribers will be attached to the events if `subscribers` is `true`.
async fn fetch_events(
    conn: &mut PgConnection,
    ids: Vec<EventId>,
    subscribers: bool,
) -> Result<Vec<Event>> {
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
    e.price_member,
    e.price_non_member,
    e.cost_per_date,
    e.location,
    e.booking_template,
    e.payment_account,
    e.alt_booking_button_text,
    e.alt_email_address,
    e.external_operator
FROM
    events e
WHERE
    e.id IN("#,
    );
    let mut separated = query_builder.separated(", ");
    for value in ids.iter() {
        separated.push_bind(value.get_ref());
    }
    query_builder.push(r#")"#);

    let mut events = Vec::new();
    for row in query_builder.build().fetch_all(&mut *conn).await? {
        events.push(map_event(&row)?);
    }

    if !events.is_empty() {
        insert_event_dates(conn, &mut events).await?;
        insert_event_custom_fields(conn, &mut events).await?;
        if subscribers {
            insert_event_subscribers(conn, &mut events).await?;
        }
    }

    Ok(events)
}

async fn insert_event_dates(conn: &mut PgConnection, events: &mut [Event]) -> Result<()> {
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
        result.entry(id).or_insert_with(Vec::new).push(date);
    }

    for event in events.iter_mut() {
        if let Some(dates) = result.remove(event.id.get_ref()) {
            event.dates = dates;
        }
    }

    Ok(())
}

async fn insert_event_custom_fields(conn: &mut PgConnection, events: &mut [Event]) -> Result<()> {
    let mut query_builder: QueryBuilder<Postgres> = QueryBuilder::new(
        r#"
SELECT
    e.id AS event_id,
	ecf.id,
	ecf.name,
	ecf.type,
	ecf.min_value,
	ecf.max_value
FROM
	events e
LEFT JOIN event_custom_fields ecf ON
	ecf.id = ANY(ARRAY[e.custom_field_1,
	e.custom_field_2,
	e.custom_field_3,
	e.custom_field_4])
WHERE
    e.id IN ("#,
    );
    let mut separated = query_builder.separated(", ");
    for event in events.iter() {
        separated.push_bind(event.id.get_ref());
    }
    separated.push_unseparated(
        r#")
	AND ecf.id IS NOT NULL
ORDER BY
    e.id"#,
    );

    let mut result = HashMap::new();
    for row in query_builder.build().fetch_all(conn).await? {
        let id: i32 = row.try_get("event_id")?;
        result
            .entry(id)
            .or_insert_with(Vec::new)
            .push(EventCustomField::new(
                row.try_get("id")?,
                row.try_get("name")?,
                row.try_get("type")?,
                row.try_get("min_value")?,
                row.try_get("max_value")?,
            ));
    }

    for event in events.iter_mut() {
        if let Some(custom_fields) = result.remove(event.id.get_ref()) {
            event.custom_fields = custom_fields;
        } else {
            event.custom_fields = Default::default();
        }
    }

    Ok(())
}

async fn insert_event_subscribers(conn: &mut PgConnection, events: &mut [Event]) -> Result<()> {
    let mut query_builder: QueryBuilder<Postgres> = QueryBuilder::new(
        r#"
SELECT
    v.event_id,
    v.id,
    v.created,
    v.first_name,
    v.last_name,
    v.street,
    v.city,
    v.email,
    v.phone,
    v.enrolled,
    v.member,
    v.payment_id,
    v.payed IS NOT NULL AS payed,
    v.comment,
    v.custom_value_1,
    v.custom_value_2,
    v.custom_value_3,
    v.custom_value_4
FROM
    v_event_bookings v
WHERE
    v.event_id IN ("#,
    );
    let mut separated = query_builder.separated(", ");
    for event in events.iter() {
        separated.push_bind(event.id.get_ref());
    }
    separated.push_unseparated(
        r#")
    AND v.canceled IS NULL
ORDER BY
    v.event_id,
    v.enrolled DESC,
    v.created"#,
    );

    let mut result = HashMap::new();
    for row in query_builder.build().fetch_all(conn).await? {
        let id: i32 = row.try_get("event_id")?;
        result
            .entry(id)
            .or_insert_with(Vec::new)
            .push(EventSubscription::new(
                row.try_get("id")?,
                row.try_get("created")?,
                row.try_get("first_name")?,
                row.try_get("last_name")?,
                row.try_get("street")?,
                row.try_get("city")?,
                row.try_get("email")?,
                row.try_get("phone")?,
                row.try_get("enrolled")?,
                row.try_get("member")?,
                row.try_get("payment_id")?,
                row.try_get("payed")?,
                row.try_get("comment")?,
                vec![
                    row.try_get::<Option<String>, _>("custom_value_1")?,
                    row.try_get::<Option<String>, _>("custom_value_2")?,
                    row.try_get::<Option<String>, _>("custom_value_3")?,
                    row.try_get::<Option<String>, _>("custom_value_4")?,
                ]
                .into_iter()
                .flatten()
                .collect(),
            ));
    }

    for event in events.iter_mut() {
        if let Some(subscribers) = result.remove(event.id.get_ref()) {
            event.subscribers = Some(subscribers);
        } else {
            event.subscribers = Some(Default::default());
        }
    }

    Ok(())
}

pub(crate) async fn write_event(
    pool: &PgPool,
    partial_event: PartialEvent,
) -> Result<(Event, Option<Vec<DateTime<Utc>>>)> {
    match partial_event.id {
        Some(id) => update_event(pool, &id, partial_event).await,
        None => Ok((save_new_event(pool, partial_event).await?, None)),
    }
}

async fn update_event(
    pool: &PgPool,
    id: &EventId,
    partial_event: PartialEvent,
) -> Result<(Event, Option<Vec<DateTime<Utc>>>)> {
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
    update_is_needed |= push_bind(&mut separated, "PRICE_MEMBER", partial_event.price_member);
    update_is_needed |= push_bind(
        &mut separated,
        "PRICE_NON_MEMBER",
        partial_event.price_non_member,
    );
    update_is_needed |= push_bind(&mut separated, "COST_PER_DATE", partial_event.cost_per_date);
    update_is_needed |= push_bind(&mut separated, "LOCATION", partial_event.location);
    update_is_needed |= push_bind(
        &mut separated,
        "BOOKING_TEMPLATE",
        partial_event.booking_template,
    );
    update_is_needed |= push_bind(
        &mut separated,
        "PAYMENT_ACCOUNT",
        partial_event.payment_account,
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
    update_is_needed |= push_bind(
        &mut separated,
        "CUSTOM_FIELD_1",
        partial_event
            .custom_fields
            .as_ref()
            .map(|fields| fields.first().map(|value| value.id)),
    );
    update_is_needed |= push_bind(
        &mut separated,
        "CUSTOM_FIELD_2",
        partial_event
            .custom_fields
            .as_ref()
            .map(|fields| fields.get(1).map(|value| value.id)),
    );
    update_is_needed |= push_bind(
        &mut separated,
        "CUSTOM_FIELD_3",
        partial_event
            .custom_fields
            .as_ref()
            .map(|fields| fields.get(2).map(|value| value.id)),
    );
    update_is_needed |= push_bind(
        &mut separated,
        "CUSTOM_FIELD_4",
        partial_event
            .custom_fields
            .as_ref()
            .map(|fields| fields.get(3).map(|value| value.id)),
    );

    // add closed date if lifecycle status should be updated to closed
    // and no closed date is defined
    let mut event_has_been_closed = false;
    if matches!(
        partial_event.lifecycle_status,
        Some(LifecycleStatus::Closed)
    ) && partial_event.closed.is_none()
    {
        event_has_been_closed = true;
        update_is_needed |= push_bind(&mut separated, "CLOSED", Some(Utc::now()));
    }

    if update_is_needed {
        query_builder.push(" WHERE id = ");
        query_builder.push_bind(id.get_ref());

        query_builder.build().execute(&mut *tx).await?;
    }

    let mut removed_dates = None;

    if let Some(new_dates) = partial_event.dates {
        let current_dates = get_event_dates(&mut tx, id).await?;
        if current_dates != new_dates {
            removed_dates = Some(
                current_dates
                    .into_iter()
                    .filter(|date| !new_dates.contains(date))
                    .collect::<Vec<_>>(),
            );
            delete_event_dates(&mut tx, id).await?;
            save_event_dates(&mut tx, id, new_dates).await?;
        }
    }

    // archive events if the event has been closed
    if event_has_been_closed {
        archive_events(&mut tx).await?;
    }

    let event = fetch_event(&mut tx, id, false)
        .await?
        .ok_or_else(|| anyhow!("Error fetching event with id '{}'", id))?;

    tx.commit().await?;

    Ok((event, removed_dates))
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
    let price_member = partial_event
        .price_member
        .ok_or_else(|| anyhow!("Attribute 'price_member' is missing"))?;
    let price_non_member = partial_event
        .price_non_member
        .ok_or_else(|| anyhow!("Attribute 'price_non_member' is missing"))?;
    let cost_per_date = partial_event.cost_per_date;
    let location = partial_event
        .location
        .ok_or_else(|| anyhow!("Attribute 'location' is missing"))?;
    let booking_template = partial_event
        .booking_template
        .ok_or_else(|| anyhow!("Attribute 'booking_template' is missing"))?;
    let payment_account = partial_event
        .payment_account
        .ok_or_else(|| anyhow!("Attribute 'payment_account' is missing"))?;
    let alt_booking_button_text = partial_event.alt_booking_button_text;
    let alt_email_address = partial_event.alt_email_address;
    let external_operator = partial_event
        .external_operator
        .ok_or_else(|| anyhow!("Attribute 'external_operator' is missing"))?;
    let (custom_field_1, custom_field_2, custom_field_3, custom_field_4) =
        match partial_event.custom_fields {
            Some(custom_fields) => (
                custom_fields.first().map(|value| value.id),
                custom_fields.get(1).map(|value| value.id),
                custom_fields.get(2).map(|value| value.id),
                custom_fields.get(3).map(|value| value.id),
            ),
            None => (None, None, None, None),
        };

    let mut tx = pool.begin().await?;

    let mut new_event: Event = query!(
        r#"
INSERT INTO events (closed, event_type, lifecycle_status, name, sort_index, short_description, description, image, light, custom_date, duration_in_minutes, max_subscribers, max_waiting_list, price_member, price_non_member, cost_per_date, location, booking_template, payment_account, alt_booking_button_text, alt_email_address, external_operator, custom_field_1, custom_field_2, custom_field_3, custom_field_4)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, $24, $25, $26)
RETURNING id, created, closed, event_type AS "event_type: EventType", lifecycle_status AS "lifecycle_status: LifecycleStatus", name, sort_index, short_description, description, image, light, custom_date, duration_in_minutes, max_subscribers, max_waiting_list, price_member, price_non_member, cost_per_date, location, booking_template, payment_account, alt_booking_button_text, alt_email_address, external_operator"#,
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
        price_member,
        price_non_member,
        cost_per_date,
        location,
        booking_template,
        payment_account,
        alt_booking_button_text,
        alt_email_address,
        external_operator,
        custom_field_1,
        custom_field_2,
        custom_field_3,
        custom_field_4,
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
            row.price_member,
            row.price_non_member,
            row.cost_per_date,
            row.location,
            row.booking_template,
            row.payment_account,
            row.alt_booking_button_text,
            row.alt_email_address,
            row.external_operator,
            Vec::new(),
        )
    })
    .fetch_one(&mut *tx)
    .await?;

    new_event.dates = save_event_dates(&mut tx, &new_event.id, dates).await?;
    new_event.custom_fields = get_event_custom_fields(&mut tx, &new_event.id).await?;

    tx.commit().await?;

    Ok(new_event)
}

async fn get_event_dates(
    conn: &mut PgConnection,
    event_id: &EventId,
) -> Result<Vec<DateTime<Utc>>> {
    let result = query!(
        r#"
SELECT
    e.date
FROM
    event_dates e
WHERE
    e.event_id = $1"#,
        event_id.get_ref()
    )
    .map(|row| row.date)
    .fetch_all(conn)
    .await?;

    Ok(result)
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

async fn save_event_dates(
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

async fn get_event_custom_fields(
    conn: &mut PgConnection,
    event_id: &EventId,
) -> Result<Vec<EventCustomField>> {
    let custom_fields = query!(
        r#"
SELECT
	ecf.id,
	ecf.name,
	ecf.type AS "cf_type: EventCustomFieldType",
	ecf.min_value,
	ecf.max_value
FROM
	events e
LEFT JOIN event_custom_fields ecf ON
	ecf.id = any(array[e.custom_field_1,
	e.custom_field_2,
	e.custom_field_3,
	e.custom_field_4])
WHERE
	e.id = $1
    AND ecf.id IS NOT NULL"#,
        event_id.get_ref()
    )
    .map(|row| EventCustomField::new(row.id, row.name, row.cf_type, row.min_value, row.max_value))
    .fetch_all(conn)
    .await?;

    Ok(custom_fields)
}

pub(crate) async fn delete_event(pool: &PgPool, id: EventId) -> Result<()> {
    let mut tx = pool.begin().await?;

    let lifecycle_status: Option<LifecycleStatus> = query!(
        r#"SELECT e.lifecycle_status AS "lifecycle_status: LifecycleStatus" FROM events e WHERE e.id = $1"#,
        id.get_ref()
    )
    .map(|row| row.lifecycle_status)
    .fetch_optional(&mut *tx)
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
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;

    Ok(())
}

/// archive all duplicate (events with the same name) closed events
/// to only have one event with the same name in status closed
async fn archive_events(conn: &mut PgConnection) -> Result<()> {
    query!(
        r#"UPDATE
	events e
SET
	lifecycle_status = 'Archived'
WHERE
	e."lifecycle_status" = 'Closed'
	AND e.name IN (
	SELECT
		name
	FROM
		events en
	WHERE
		en."lifecycle_status" = 'Closed'
	GROUP BY
		en.name
	HAVING
		COUNT(*) > 1
)
	AND e.closed < (
	SELECT
		MAX(closed)
	FROM
		events ec
	WHERE
		ec.name = e.name
)"#
    )
    .execute(conn)
    .await?;

    Ok(())
}

/// return a list of events who starts next week
/// and had no reminder email sent until now
pub(crate) async fn get_reminder_events(pool: &PgPool) -> Result<Vec<Event>> {
    let mut conn = pool.acquire().await?;

    let event_ids: Vec<EventId> = query!(
        r#"
SELECT
	e.id
FROM
	events e
INNER JOIN (
	SELECT
		ied.event_id,
		MIN(ied.date) as date
	FROM
		event_dates ied
	GROUP BY
		ied.event_id) ed ON
	e.id = ed.event_id
WHERE
	e.reminder_sent IS NULL
	AND e.lifecycle_status IN('Review', 'Published', 'Running')
	AND ed.date >= (CURRENT_DATE + INTERVAL '1' DAY)
	AND ed.date <= (CURRENT_DATE + INTERVAL '6' DAY)
	AND EXISTS (
	SELECT
		*
	FROM
		event_bookings eb
	WHERE
		e.id = eb.event_id
		AND eb.enrolled IS TRUE
		AND eb.canceled IS NULL)"#
    )
    .map(|row| row.id.into())
    .fetch_all(&mut *conn)
    .await?;

    let events = if !event_ids.is_empty() {
        fetch_events(&mut conn, event_ids, true).await?
    } else {
        vec![]
    };

    Ok(events)
}

/// mark the given event that the reminder email has been sent
/// (to avoid duplicate sending of reminder emails)
pub(crate) async fn mark_as_reminder_sent(pool: &PgPool, event_id: &EventId) -> Result<()> {
    query!(
        r#"
UPDATE
    events
SET
    reminder_sent = NOW()
WHERE
    id = $1"#,
        event_id.get_ref(),
    )
    .execute(pool)
    .await?;

    Ok(())
}

/// Return the id's of all events in status 'Running'
/// where all event dates are in the past and every booking
/// is payed.
pub(crate) async fn get_all_finished_event_ids(pool: &PgPool) -> Result<Vec<EventId>> {
    let mut conn = pool.acquire().await?;

    let event_ids: Vec<EventId> = query!(
        r#"SELECT
    e.id
FROM
    events e
WHERE
    e.lifecycle_status = 'Running'
    AND e.custom_date IS NULL
    AND (
    SELECT
        MAX(ed.date)
    FROM
        event_dates ed
    WHERE
        ed.event_id = e.id) < NOW()
    AND NOT EXISTS (
    SELECT
        1
    FROM
        event_bookings eb
    WHERE
        eb.event_id = e.id
        AND eb.enrolled IS TRUE
        AND eb.canceled IS NULL
        AND eb.payed IS NULL)"#
    )
    .map(|row| row.id.into())
    .fetch_all(&mut *conn)
    .await?;

    Ok(event_ids)
}

fn map_event(row: &PgRow) -> Result<Event> {
    Ok(Event::new(
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
        row.try_get("price_member")?,
        row.try_get("price_non_member")?,
        row.try_get("cost_per_date")?,
        row.try_get("location")?,
        row.try_get("booking_template")?,
        row.try_get("payment_account")?,
        row.try_get("alt_booking_button_text")?,
        row.try_get("alt_email_address")?,
        row.try_get("external_operator")?,
        Vec::new(),
    ))
}

#[cfg(test)]
mod db_integration_tests {
    use super::*;
    use crate::models::{EventType, LifecycleStatus, PartialEvent};
    use anyhow::Result;
    use chrono::Utc;

    #[sqlx::test]
    async fn test_write_and_get_event(pool: PgPool) -> Result<()> {
        let partial = PartialEvent {
            event_type: Some(EventType::Events),
            lifecycle_status: Some(LifecycleStatus::Draft),
            name: Some("Test Event".to_string()),
            sort_index: Some(1),
            short_description: Some("Short desc".to_string()),
            description: Some("Full description".to_string()),
            image: Some("test.png".to_string()),
            light: Some(false),
            duration_in_minutes: Some(120),
            max_subscribers: Some(50),
            max_waiting_list: Some(10),
            price_member: Some("25.00".parse().unwrap()),
            price_non_member: Some("30.00".parse().unwrap()),
            location: Some("Gym".to_string()),
            booking_template: Some("Template".to_string()),
            payment_account: Some("Account".to_string()),
            external_operator: Some(false),
            dates: Some(vec![Utc::now()]),
            ..Default::default()
        };

        let (event, _) = write_event(&pool, partial).await?;
        assert_eq!(event.name, "Test Event");
        assert_eq!(event.event_type, EventType::Events);
        assert_eq!(event.lifecycle_status, LifecycleStatus::Draft);

        let fetched: Option<Event> = get_event(&pool, &event.id, false).await?;
        assert!(fetched.is_some());
        let fetched = fetched.unwrap();
        assert_eq!(fetched.id, event.id);
        assert_eq!(fetched.name, "Test Event");

        let partial = PartialEvent {
            id: Some(event.id),
            lifecycle_status: Some(LifecycleStatus::Published),
            name: Some("Updated Event".to_string()),
            ..Default::default()
        };

        let (updated, _) = write_event(&pool, partial).await?;
        assert_eq!(updated.name, "Updated Event");
        assert_eq!(updated.lifecycle_status, LifecycleStatus::Published);

        Ok(())
    }
}
