use super::calendar;
use crate::email;
use log::{error, info};

pub async fn check_email_connectivity() {
    match email::test_connection().await {
        Ok(_) => info!("Email connectivity checks finished successfully"),
        Err(e) => error!("Email connectivity checks failed: {}", e),
    }
}

pub async fn renew_calendar_watch() {
    match calendar::renew_watch().await {
        Ok(_) => info!("Calendar watch has been renewed"),
        Err(e) => error!("Error renewing calendar watch: {}", e),
    }
}
