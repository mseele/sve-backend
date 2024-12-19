use crate::{calendar, models::Appointment};
use anyhow::Result;
use hyper::StatusCode;
use reqwest::Client;
use tracing::{info, warn};

const RE_DEPLOY_HOOK: &str = "https://api.netlify.com/build_hooks/66fd9717537e8d6941f92c34";

const GENERAL_ID: &str = "info@sv-eutingen.de";

const WATCH_ID: &str = "01234567-89ab-cdef-0123456789ab";

const WATCH_RESOURCE_ID: &str = "9-xc9GFSc2LvPpsJiw8HveIDA3c";

pub(crate) async fn appointments() -> Result<Vec<Appointment>> {
    calendar::appointments(GENERAL_ID, 100).await
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

pub(crate) async fn renew_watch() -> Result<()> {
    calendar::renew_watch(GENERAL_ID, WATCH_ID, WATCH_RESOURCE_ID).await
}
