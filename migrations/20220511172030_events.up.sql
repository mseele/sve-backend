CREATE TYPE lifecycle_status AS ENUM (
    'Draft',
    'Review',
    'Published',
    'Finished',
    'Closed'
);

CREATE TYPE event_type AS ENUM ('Fitness', 'Events');

CREATE SEQUENCE payment_id AS SMALLINT MINVALUE 1000 MAXVALUE 9999 CYCLE INCREMENT BY 1;

CREATE TABLE events (
    id SERIAL PRIMARY KEY,
    created TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    closed TIMESTAMP WITH TIME ZONE,
    event_type EVENT_TYPE NOT NULL,
    lifecycle_status LIFECYCLE_STATUS NOT NULL,
    name TEXT NOT NULL,
    sort_index SMALLINT NOT NULL,
    short_description TEXT NOT NULL,
    description TEXT NOT NULL,
    image TEXT NOT NULL,
    light BOOLEAN NOT NULL,
    custom_date TEXT,
    duration_in_minutes SMALLINT NOT NULL,
    max_subscribers SMALLINT NOT NULL,
    max_waiting_list SMALLINT NOT NULL,
    cost_member DECIMAL(12, 2) NOT NULL,
    cost_non_member DECIMAL(12, 2) NOT NULL,
    location TEXT NOT NULL,
    booking_template TEXT NOT NULL,
    waiting_template TEXT NOT NULL,
    alt_booking_button_text TEXT,
    alt_email_address TEXT,
    external_operator BOOLEAN NOT NULL
);

CREATE TABLE event_dates (
    event_id INTEGER NOT NULL REFERENCES events (id),
    date TIMESTAMP WITH TIME ZONE NOT NULL
);

CREATE TABLE event_subscribers (
    id SERIAL PRIMARY KEY,
    first_name TEXT NOT NULL,
    last_name TEXT NOT NULL,
    street TEXT NOT NULL,
    city TEXT NOT NULL,
    email TEXT NOT NULL,
    phone TEXT,
    member BOOLEAN NOT NULL
);

CREATE TABLE event_bookings (
    id SERIAL PRIMARY KEY,
    event_id INTEGER NOT NULL REFERENCES events (id),
    created TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    canceled TIMESTAMP WITH TIME ZONE,
    enrolled BOOLEAN NOT NULL,
    pre_booking BOOLEAN NOT NULL,
    subscriber_id INTEGER NOT NULL REFERENCES event_subscribers (id),
    comment TEXT,
    payment_id TEXT NOT NULL UNIQUE,
    payed TIMESTAMP WITH TIME ZONE,
    iban TEXT
);

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