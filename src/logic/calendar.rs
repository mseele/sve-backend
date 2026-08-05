use crate::calendar::CalendarClient;
use crate::models::Appointment;
use anyhow::Result;
use async_trait::async_trait;
use hyper::StatusCode;
use reqwest::Client;
use tracing::{info, warn};

#[cfg(test)]
use mockall::automock;

const GENERAL_ID: &str = "info@sv-eutingen.de";

const WATCH_ID: &str = "01234567-89ab-cdef-0123456789ab";

const WATCH_RESOURCE_ID: &str = "9-xc9GFSc2LvPpsJiw8HveIDA3c";

#[async_trait]
#[cfg_attr(test, automock)]
pub(crate) trait DeployHook {
    async fn trigger(&self) -> Result<StatusCode>;
}

pub(crate) struct NetlifyDeployHook {
    client: Client,
}

impl NetlifyDeployHook {
    pub(crate) fn new(client: Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl DeployHook for NetlifyDeployHook {
    async fn trigger(&self) -> Result<StatusCode> {
        const URL: &str = "https://api.netlify.com/build_hooks/66fd9717537e8d6941f92c34";
        let resp = self.client.post(URL).send().await?;
        Ok(resp.status())
    }
}

pub(crate) async fn appointments(client: &CalendarClient) -> Result<Vec<Appointment>> {
    client.appointments(GENERAL_ID, 100).await
}

pub(crate) async fn notifications(channel_id: &str, hook: &impl DeployHook) -> Result<()> {
    info!(
        "Recieved calendar notification for channel id {}",
        channel_id
    );

    let status = hook.trigger().await?;

    if status == StatusCode::OK {
        info!("Re-Deploy triggered successfully");
    } else {
        warn!(
            "Trigger Re-Deploy failed with status code {}",
            status
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
    use crate::logic::secrets::ConsolidatedAwsSecretProvider;
    use std::sync::Arc;

    fn init_crypto() {
        dotenvy::dotenv().ok();
        let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
    }

    #[tokio::test]
    async fn test_notifications_success() {
        let mut mock = MockDeployHook::new();
        mock.expect_trigger()
            .returning(|| Box::pin(async { Ok(StatusCode::OK) }));
        let result = notifications("test-channel", &mock).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_notifications_non_2xx() {
        let mut mock = MockDeployHook::new();
        mock.expect_trigger()
            .returning(|| Box::pin(async { Ok(StatusCode::INTERNAL_SERVER_ERROR) }));
        let result = notifications("test-channel", &mock).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_appointments() {
        init_crypto();

        let client = CalendarClient::new(Arc::new(ConsolidatedAwsSecretProvider::new()));
        let result = appointments(&client).await;
        assert!(result.is_ok(), "appointments() failed: {:?}", result.err());
    }

    #[tokio::test]
    async fn test_renew_watch() {
        init_crypto();

        let client = CalendarClient::new(Arc::new(ConsolidatedAwsSecretProvider::new()));
        let result = renew_watch(&client).await;
        assert!(result.is_ok(), "renew_watch() failed: {:?}", result.err());
    }
}
