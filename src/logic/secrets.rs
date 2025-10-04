//! Secret management abstraction for sve-backend
//
// This module provides functions to load secrets from environment variables (for local/dev)
// and from AWS Secrets Manager in production.

use anyhow::{Result, anyhow};
use aws_config::{BehaviorVersion, Region};
use aws_sdk_secretsmanager::Client;
use mini_moka::sync::Cache;
use once_cell::sync::Lazy;
use std::sync::Arc;
use std::{env, time::Duration};
use tokio::sync::Mutex;

static AWS_CLIENT: Lazy<Arc<Mutex<Option<Client>>>> = Lazy::new(|| Arc::new(Mutex::new(None)));

static SECRET_CACHE: Lazy<Cache<String, String>> = Lazy::new(|| {
    Cache::builder()
        .time_to_live(Duration::from_secs(300)) // 5 minutes
        .build()
});

/// Loads a secret from an environment variable if present, otherwise from AWS Secrets Manager.
pub async fn get(key: &str) -> Result<String> {
    // 1. Try env var
    if let Ok(val) = env::var(key) {
        return Ok(val);
    }

    // 2. Try cache
    if let Some(value) = SECRET_CACHE.get(&key.to_string()) {
        return Ok(value);
    }

    // 3. Fetch from AWS and cache
    let secret = get_aws_secret(key).await?;
    SECRET_CACHE.insert(key.to_string(), secret.clone());
    Ok(secret)
}

async fn get_aws_secret(key: &str) -> Result<String> {
    let mut client_guard = AWS_CLIENT.lock().await;

    // Initialize client if needed
    if client_guard.is_none() {
        let config = aws_config::defaults(BehaviorVersion::latest())
            .region(Region::new(
                env::var("AWS_REGION").unwrap_or_else(|_| "eu-central-1".to_string()),
            ))
            .load()
            .await;
        *client_guard = Some(Client::new(&config));
    }

    // Safe to unwrap as we just ensured it exists, but let's be defensive anyway
    let client = client_guard
        .as_ref()
        .ok_or_else(|| anyhow!("Failed to initialize AWS client"))?;

    let resp = client
        .get_secret_value()
        .secret_id(key)
        .send()
        .await
        .map_err(|e| anyhow!("AWS SecretsManager error: {e}"))?;

    resp.secret_string()
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow!("Secret {key} not found"))
}
