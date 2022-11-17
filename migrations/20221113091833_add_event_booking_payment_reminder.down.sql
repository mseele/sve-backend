DROP VIEW v_event_counters;
DROP VIEW v_event_bookings;

ALTER TABLE
    event_bookings DROP COLUMN payment_reminder_sent;

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
