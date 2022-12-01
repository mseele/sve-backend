ALTER TABLE
    events
ADD
    cost_per_date DECIMAL(12, 2) NULL;

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