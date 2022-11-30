ALTER TABLE
    events
ADD
    cost_member DECIMAL(12, 2) NULL,
ADD
    cost_non_member DECIMAL(12, 2) NULL;

UPDATE
    events
SET
    cost_member = price_member,
    cost_non_member = price_non_member;

DROP VIEW v_events;

ALTER TABLE
    events
ALTER COLUMN
    cost_member SET NOT NULL,
ALTER COLUMN
    cost_non_member SET NOT NULL,
DROP COLUMN
    price_member,
DROP COLUMN
    price_non_member;

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