DROP VIEW v_events;

CREATE VIEW v_events AS
SELECT
    e.*,
    ed.date
FROM
    events e
LEFT JOIN event_dates ed ON
    e.id = ed.event_id
ORDER BY
    e.created,
    ed.date;
