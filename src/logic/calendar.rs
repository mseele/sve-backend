use crate::calendar::CalendarClient;
use crate::models::Appointment;
use anyhow::Result;
use hyper::StatusCode;
use reqwest::Client;
use tracing::{info, warn};

const RE_DEPLOY_HOOK: &str = "https://api.netlify.com/build_hooks/66fd9717537e8d6941f92c34";

const GENERAL_ID: &str = "info@sv-eutingen.de";

const WATCH_ID: &str = "01234567-89ab-cdef-0123456789ab";

const WATCH_RESOURCE_ID: &str = "9-xc9GFSc2LvPpsJiw8HveIDA3c";

pub(crate) async fn appointments(client: &CalendarClient) -> Result<Vec<Appointment>> {
    client.appointments(GENERAL_ID, 100).await
}

pub(crate) async fn notifications(channel_id: &str) -> Result<()> {
    info!(
        "Recieved calendar notification for channel id {}",
        channel_id
    );

    let resp = Client::new().post(RE_DEPLOY_HOOK).send().await?;

    if resp.status() == StatusCode::OK {
        info!("Re-Deploy triggered successfully");
    } else {
        warn!(
            "Trigger Re-Deploy failed with status code {}",
            resp.status()
        );
    }

    Ok(())
}

pub(crate) async fn renew_watch(client: &CalendarClient) -> Result<()> {
    client
        .renew_watch(GENERAL_ID, WATCH_ID, WATCH_RESOURCE_ID)
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logic::secrets::EnvSecretProvider;
    use std::sync::Arc;

    fn init_crypto() {
        dotenvy::dotenv().ok();
        let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
    }

    #[tokio::test]
    async fn test_appointments() {
        init_crypto();

        let client = CalendarClient::new(Arc::new(EnvSecretProvider));
        let result = appointments(&client).await;
        assert!(result.is_ok(), "appointments() failed: {:?}", result.err());
    }

    #[tokio::test]
    async fn test_renew_watch() {
        init_crypto();

        let client = CalendarClient::new(Arc::new(EnvSecretProvider));
        let result = renew_watch(&client).await;
        assert!(result.is_ok(), "renew_watch() failed: {:?}", result.err());
    }
}
