ALTER TABLE
    event_custom_fields
ADD COLUMN price_relevant BOOLEAN NOT NULL DEFAULT FALSE;

ALTER TABLE event_custom_fields
ADD CONSTRAINT price_relevant_requires_number_type
CHECK (NOT price_relevant OR type = 'Number');