//! Secret management abstraction for sve-backend
//
// This module provides functions to load secrets from environment variables (for local/dev)
// and from AWS Secrets Manager in production.

use anyhow::{Result, anyhow};
use aws_config::{BehaviorVersion, Region};
use aws_sdk_secretsmanager::Client;
use mini_moka::sync::Cache;
use once_cell::sync::Lazy;
use serde::Deserialize;
use std::sync::Arc;
use std::{env, time::Duration};
use tokio::sync::Mutex;

static AWS_CLIENT: Lazy<Arc<Mutex<Option<Client>>>> = Lazy::new(|| Arc::new(Mutex::new(None)));

static SECRET_CACHE: Lazy<Cache<String, Secrets>> = Lazy::new(|| {
    Cache::builder()
        .time_to_live(Duration::from_secs(900)) // 15 minutes
        .build()
});

const CONSOLIDATED_SECRET_NAME: &str = "sve-backend";

#[derive(Deserialize, Clone)]
struct Secrets {
    #[serde(rename = "DATABASE_URL")]
    pub database_url: String,
    #[serde(rename = "TASK_API_KEY")]
    pub task_api_key: String,
    #[serde(rename = "CAPTCHA_SECRET")]
    pub captcha_secret: String,
    #[serde(rename = "GOOGLE_CREDS")]
    pub google_creds: String,
    #[serde(rename = "EMAIL_ACCOUNTS")]
    pub email_accounts: String,
}

/// Loads a secret from an environment variable if present, otherwise from AWS Secrets Manager.
pub async fn get(key: &str) -> Result<String> {
    if let Ok(val) = env::var(key) {
        return Ok(val);
    }

    let secrets = get_consolidated_secrets().await?;

    match key {
        "DATABASE_URL" => Ok(secrets.database_url),
        "TASK_API_KEY" => Ok(secrets.task_api_key),
        "CAPTCHA_SECRET" => Ok(secrets.captcha_secret),
        "GOOGLE_CREDS" => Ok(secrets.google_creds),
        "EMAIL_ACCOUNTS" => Ok(secrets.email_accounts),
        _ => Err(anyhow!("Unknown secret key: {}", key)),
    }
}

async fn get_consolidated_secrets() -> Result<Secrets> {
    if let Some(secrets) = SECRET_CACHE.get(&CONSOLIDATED_SECRET_NAME.to_string()) {
        return Ok(secrets);
    }

    let secrets = fetch_consolidated_secret(CONSOLIDATED_SECRET_NAME).await?;
    SECRET_CACHE.insert(CONSOLIDATED_SECRET_NAME.to_string(), secrets.clone());
    Ok(secrets)
}

async fn fetch_consolidated_secret(secret_id: &str) -> Result<Secrets> {
    let mut client_guard = AWS_CLIENT.lock().await;

    if client_guard.is_none() {
        let config = aws_config::defaults(BehaviorVersion::latest())
            .region(Region::new(
                env::var("AWS_REGION").unwrap_or_else(|_| "eu-central-1".to_string()),
            ))
            .load()
            .await;
        *client_guard = Some(Client::new(&config));
    }

    let client = client_guard
        .as_ref()
        .ok_or_else(|| anyhow!("Failed to initialize AWS client"))?;

    let resp = client
        .get_secret_value()
        .secret_id(secret_id)
        .send()
        .await
        .map_err(|e| anyhow!("AWS SecretsManager error: {e}"))?;

    let secret_string = resp
        .secret_string()
        .ok_or_else(|| anyhow!("Secret {} not found", secret_id))?;

    serde_json::from_str(secret_string)
        .map_err(|e| anyhow!("Failed to parse consolidated secret: {e}"))
}
