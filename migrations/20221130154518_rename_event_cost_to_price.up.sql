ALTER TABLE
    events
ADD
    price_member DECIMAL(12, 2) NULL,
ADD
    price_non_member DECIMAL(12, 2) NULL;

UPDATE
    events
SET
    price_member = cost_member,
    price_non_member = cost_non_member;

DROP VIEW v_events;

ALTER TABLE
    events
ALTER COLUMN
    price_member SET NOT NULL,
ALTER COLUMN
    price_non_member SET NOT NULL,
DROP COLUMN
    cost_member,
DROP COLUMN
    cost_non_member;

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