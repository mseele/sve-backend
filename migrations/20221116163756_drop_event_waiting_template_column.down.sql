ALTER TABLE
    events
ADD
    waiting_template TEXT NULL;

UPDATE
    events
SET
    waiting_template = '';

ALTER TABLE
    events
ALTER COLUMN
    waiting_template
SET
    NOT NULL;

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