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
