UPDATE
    events
SET
    payment_account = '-'
WHERE payment_account IS NULL;

ALTER TABLE
    events
ALTER COLUMN
    payment_account
SET
    NOT NULL;
