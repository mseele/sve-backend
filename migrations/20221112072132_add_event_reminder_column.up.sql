ALTER TABLE
    events
ADD
    reminder_sent TIMESTAMP WITH TIME ZONE;

DROP VIEW v_events;

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