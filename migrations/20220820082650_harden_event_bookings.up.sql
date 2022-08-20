ALTER TABLE
    event_bookings
ADD
    CONSTRAINT event_subscribers_ukey UNIQUE (event_id, subscriber_id);