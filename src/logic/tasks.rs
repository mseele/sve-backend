use super::calendar;
use super::events;
use crate::email;
use log::{error, info};
use sqlx::PgPool;

pub(crate) async fn check_email_connectivity() {
    match email::test_connection().await {
        Ok(_) => info!("Email connectivity checks finished successfully"),
        Err(e) => error!("Email connectivity checks failed: {}", e),
    }
}

pub(crate) async fn renew_calendar_watch() {
    match calendar::renew_watch().await {
        Ok(_) => info!("Calendar watch has been renewed"),
        Err(e) => error!("Error renewing calendar watch: {}", e),
    }
}

/// send a reminder email for each events that starts next week
pub(crate) async fn send_event_reminders(pool: &PgPool) {
    match events::send_event_reminders(pool).await {
        Ok(count) if count > 0 => info!("{} event reminders has been send successfully.", count),
        Ok(_) => (),
        Err(e) => error!("Error while sending event reminders: {}", e),
    }
}
