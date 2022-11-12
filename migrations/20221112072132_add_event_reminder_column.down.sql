DROP VIEW v_events;

ALTER TABLE
    events DROP COLUMN reminder_sent;

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