CREATE TABLE event_manager_recipients (
    event_id INTEGER NOT NULL REFERENCES events (id) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    email TEXT NOT NULL,
    PRIMARY KEY (event_id, position)
);
