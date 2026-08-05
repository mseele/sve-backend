use crate::calendar::CalendarApi;
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
    pub(crate) fn new() -> Self {
        Self {
            client: Client::new(),
        }
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

pub(crate) async fn appointments(api: &dyn CalendarApi) -> Result<Vec<Appointment>> {
    api.list_events(GENERAL_ID, 100).await
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

pub(crate) async fn renew_watch(api: &dyn CalendarApi) -> Result<()> {
    api.renew_watch(GENERAL_ID, WATCH_ID, WATCH_RESOURCE_ID)
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calendar::InMemoryCalendarApi;
    use crate::models::Appointment;
    use chrono::NaiveDate;
    use pretty_assertions::assert_eq;

    fn sample_appointments() -> Vec<Appointment> {
        vec![Appointment::new(
            Some("evt-1".into()),
            0,
            Some("Test Event".into()),
            None,
            None,
            Some(NaiveDate::from_ymd_opt(2026, 8, 10).unwrap()),
            Some(NaiveDate::from_ymd_opt(2026, 8, 10).unwrap()),
            None,
            None,
        )]
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
        let api = InMemoryCalendarApi::new(sample_appointments());
        let result = appointments(&api).await;
        assert!(result.is_ok(), "appointments() failed: {:?}", result.err());
        let apps = result.unwrap();
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].title.as_deref(), Some("Test Event"));
    }

    #[tokio::test]
    async fn test_renew_watch() {
        let api = InMemoryCalendarApi::new(Vec::new());
        let result = renew_watch(&api).await;
        assert!(result.is_ok(), "renew_watch() failed: {:?}", result.err());
    }

    #[tokio::test]
    async fn test_list_events_returns_canned_appointments() {
        let api = InMemoryCalendarApi::new(sample_appointments());
        let result = api.list_events("any-calendar", 50).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_list_events_empty() {
        let api = InMemoryCalendarApi::new(Vec::new());
        let result = api.list_events("any-calendar", 50).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_renew_watch_error() {
        let api = InMemoryCalendarApi::new(Vec::new()).with_watch_error("watch failed");
        let result = renew_watch(&api).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_stop_watch_succeeds_by_default() {
        let api = InMemoryCalendarApi::new(Vec::new());
        let result = api.stop_watch("id", "res").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_stop_watch_returns_error_when_pushed() {
        let api = InMemoryCalendarApi::new(Vec::new());
        api.push_stop_watch_error("channel expired");
        let result = api.stop_watch("id", "res").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("channel expired"));
    }
}
