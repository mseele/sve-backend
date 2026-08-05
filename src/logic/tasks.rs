use anyhow::Result;
use sqlx::PgPool;
use tracing::{error, info};

use super::{calendar, events};
use crate::calendar::CalendarClient;
use crate::email::{EmailGateway, RealEmailGateway};
use crate::models::{EventId, EventType};

pub(crate) async fn check_email_connectivity(email_gateway: &RealEmailGateway) {
    match email_gateway.test_connection().await {
        Ok(_) => info!("Email connectivity checks finished successfully"),
        Err(e) => error!("Email connectivity checks failed: {}", e),
    }
}

pub(crate) async fn renew_calendar_watch(client: &CalendarClient) {
    match calendar::renew_watch(client).await {
        Ok(_) => info!("Calendar watch has been renewed"),
        Err(e) => error!("Error renewing calendar watch: {}", e),
    }
}

/// send a reminder email for all events starting next week
pub(crate) async fn send_event_reminders(pool: &PgPool, email_gateway: &impl EmailGateway) {
    match events::send_event_reminders(pool, email_gateway).await {
        Ok(count) if count > 0 => info!("{} event reminders has been send successfully.", count),
        Ok(_) => (),
        Err(e) => error!("Error while sending event reminders: {}", e),
    }
}

/// Complete all finished events.
pub(crate) async fn close_finished_events(pool: &PgPool, email_gateway: &impl EmailGateway) {
    match events::close_finished_running_events(pool, email_gateway).await {
        Ok(count) if count > 0 => info!("{count} events has been closed."),
        Ok(_) => (),
        Err(e) => error!("Error while closing finished events: {}", e),
    }
}

/// send a reminder email for all bookings which are due with payment
pub(crate) async fn send_payment_reminders(
    pool: &PgPool,
    event_type: EventType,
    email_gateway: &impl EmailGateway,
) -> Result<()> {
    match events::send_payment_reminders(pool, event_type, email_gateway).await {
        Ok(count) if count > 0 => {
            info!("{} payment reminders has been send successfully.", count);
            Ok(())
        }
        Ok(_) => Ok(()),
        Err(e) => {
            error!("Error while sending payment reminders: {}", e);
            Err(e)
        }
    }
}

/// send participation confirmation after finished event
pub(crate) async fn send_participation_confirmation(
    pool: &PgPool,
    event_id: EventId,
    email_gateway: &impl EmailGateway,
) -> Result<()> {
    match events::send_participation_confirmation(pool, event_id, email_gateway).await {
        Ok(count) if count > 0 => {
            info!(
                "{} participation confirmations has been send successfully.",
                count
            );
            Ok(())
        }
        Ok(_) => Ok(()),
        Err(e) => {
            error!("Error while sending participation confirmations: {}", e);
            Err(e)
        }
    }
}
