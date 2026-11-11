CREATE TABLE event_manager_reminder_sends (
    event_id INTEGER NOT NULL REFERENCES events (id) ON DELETE CASCADE,
    lead_days INTEGER NOT NULL CHECK (lead_days >= 0),
    sent_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    PRIMARY KEY (event_id, lead_days)
);
