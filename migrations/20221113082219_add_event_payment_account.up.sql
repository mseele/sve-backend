ALTER TABLE
    events
ADD
    payment_account TEXT NULL;

UPDATE
    events
SET
    payment_account = 'payment_account_to_add';

ALTER TABLE
    events
ALTER COLUMN
    payment_account
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