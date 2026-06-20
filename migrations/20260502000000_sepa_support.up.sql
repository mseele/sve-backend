-- Create payment_method enum
CREATE TYPE payment_method AS ENUM ('BankTransfer', 'SepaDirectDebit');

-- Add payment_method to events (default BankTransfer = current behavior)
ALTER TABLE events ADD COLUMN payment_method PAYMENT_METHOD NOT NULL DEFAULT 'BankTransfer';

-- Rename payed to payment_confirmed_at (clear semantics, fixes typo)
ALTER TABLE event_bookings RENAME COLUMN payed TO payment_confirmed_at;

-- Add sepa_exported_at to event_bookings (permanent export mark)
ALTER TABLE event_bookings ADD COLUMN sepa_exported_at TIMESTAMP WITH TIME ZONE;

-- Document iban dual semantics
COMMENT ON COLUMN event_bookings.iban IS 'Dual semantics by events.payment_method: BankTransfer = payer IBAN from CSV verification, SepaDirectDebit = debtor IBAN from booking form. Always join events.payment_method when reading this column.';

-- Recreate v_event_bookings view after column rename
-- Postgres expands eb.* at CREATE time; the existing view will error after the rename
DROP VIEW IF EXISTS v_event_bookings CASCADE;
CREATE VIEW v_event_bookings AS
SELECT eb.*, es.first_name, es.last_name, es.street, es.city, es.email, es.phone, es.member
FROM event_bookings eb, event_subscribers es
WHERE eb.subscriber_id = es.id
ORDER BY eb.created, eb.enrolled;

-- Recreate v_event_counters (depends on v_event_bookings, dropped by CASCADE)
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
