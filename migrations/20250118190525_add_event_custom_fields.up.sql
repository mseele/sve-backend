CREATE TYPE event_cf_type AS ENUM (
    'Text',
    'Number'
);

CREATE TABLE event_custom_fields (
    id SERIAL PRIMARY KEY,
    name TEXT NOT NULL,
    type event_cf_type NOT NULL,
    min_value INTEGER,
    max_value INTEGER
);

ALTER TABLE
    events
ADD COLUMN custom_field_1 INTEGER REFERENCES event_custom_fields (id),
ADD COLUMN custom_field_2 INTEGER REFERENCES event_custom_fields (id),
ADD COLUMN custom_field_3 INTEGER REFERENCES event_custom_fields (id),
ADD COLUMN custom_field_4 INTEGER REFERENCES event_custom_fields (id);

ALTER TABLE
    event_bookings
ADD COLUMN custom_value_1 TEXT,
ADD COLUMN custom_value_2 TEXT,
ADD COLUMN custom_value_3 TEXT,
ADD COLUMN custom_value_4 TEXT;

DROP VIEW v_events;
DROP VIEW v_event_counters;
DROP VIEW v_event_bookings;

CREATE VIEW v_events AS
SELECT
    e.*,
    ed.date
FROM
    events e,
    event_dates ed
WHERE
    e.id = ed.event_id
ORDER BY
    e.created,
    ed.date;

CREATE VIEW v_event_bookings AS
SELECT
    eb.*,
    es.first_name,
    es.last_name,
    es.street,
    es.city,
    es.email,
    es.phone,
    es.member
FROM
    event_bookings eb,
    event_subscribers es
WHERE
    eb.subscriber_id = es.id
ORDER BY
    eb.created,
    eb.enrolled;

CREATE VIEW v_event_counters AS
SELECT
	e.id,
	e.max_subscribers,
	(
	SELECT
		COUNT(*)
	FROM
		v_event_bookings v
	WHERE
		e.id = v.event_id
		AND v.canceled IS NULL
		AND v.enrolled IS TRUE) AS subscribers,
	e.max_waiting_list,
	(
	SELECT
		COUNT(*)
	FROM
		v_event_bookings v
	WHERE
		e.id = v.event_id
		AND v.canceled IS NULL
		AND v.enrolled IS FALSE) AS waiting_list
FROM
	events e;