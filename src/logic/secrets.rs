//! Secret management abstraction for sve-backend.
//
// Provides a `SecretProvider` seam with two adapters:
// - `EnvSecretProvider` reads secrets from environment variables (local/dev).
// - `ConsolidatedAwsSecretProvider` reads a consolidated secret from AWS
//   Secrets Manager, falling back to environment variables first so local dev
//   keeps working via `.env`. Caching is owned by the adapter, not a global.

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use aws_config::{BehaviorVersion, Region};
use aws_sdk_secretsmanager::Client;
use mini_moka::sync::Cache;
#[cfg(test)]
use mockall::automock;
use serde::Deserialize;
use std::{env, time::Duration};

const CONSOLIDATED_SECRET_NAME: &str = "sve-backend";

/// Typed key for every secret the application reads.
/// Replaces raw `&str` dispatch so call sites and the provider
/// agree on the vocabulary at compile time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SecretKey {
    DatabaseUrl,
    TaskApiKey,
    CaptchaSecret,
    GoogleCreds,
    EmailAccounts,
    SepaCreditorName,
    SepaCreditorIban,
    SessionSecret,
}

impl SecretKey {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::DatabaseUrl => "DATABASE_URL",
            Self::TaskApiKey => "TASK_API_KEY",
            Self::CaptchaSecret => "CAPTCHA_SECRET",
            Self::GoogleCreds => "GOOGLE_CREDS",
            Self::EmailAccounts => "EMAIL_ACCOUNTS",
            Self::SepaCreditorName => "SEPA_CREDITOR_NAME",
            Self::SepaCreditorIban => "SEPA_CREDITOR_IBAN",
            Self::SessionSecret => "SESSION_SECRET",
        }
    }
}

#[async_trait]
#[cfg_attr(test, automock)]
pub(crate) trait SecretProvider: Send + Sync {
    async fn get(&self, key: SecretKey) -> Result<String>;
}

/// Reads secrets from environment variables only.
pub(crate) struct EnvSecretProvider;

#[async_trait]
impl SecretProvider for EnvSecretProvider {
    async fn get(&self, key: SecretKey) -> Result<String> {
        env::var(key.as_str()).map_err(|_| anyhow!("Missing env var: {}", key.as_str()))
    }
}

#[derive(Deserialize, Clone)]
struct Secrets {
    #[serde(rename = "DATABASE_URL")]
    database_url: String,
    #[serde(rename = "TASK_API_KEY")]
    task_api_key: String,
    #[serde(rename = "CAPTCHA_SECRET")]
    captcha_secret: String,
    #[serde(rename = "GOOGLE_CREDS")]
    google_creds: String,
    #[serde(rename = "EMAIL_ACCOUNTS")]
    email_accounts: String,
    #[serde(default, rename = "SEPA_CREDITOR_NAME")]
    sepa_creditor_name: String,
    #[serde(default, rename = "SEPA_CREDITOR_IBAN")]
    sepa_creditor_iban: String,
    #[serde(rename = "SESSION_SECRET")]
    session_secret: String,
}

/// Reads the consolidated AWS secret, falling back to environment variables.
/// Owns its own cache and lazily-initialized AWS client.
pub(crate) struct ConsolidatedAwsSecretProvider {
    env: EnvSecretProvider,
    cache: Cache<String, Secrets>,
    client: tokio::sync::OnceCell<Client>,
}

impl ConsolidatedAwsSecretProvider {
    pub(crate) fn new() -> Self {
        Self {
            env: EnvSecretProvider,
            cache: Cache::builder()
                .time_to_live(Duration::from_secs(900)) // 15 minutes
                .build(),
            client: tokio::sync::OnceCell::new(),
        }
    }

    async fn client(&self) -> Result<&Client> {
        self.client
            .get_or_try_init(|| async {
                let config = aws_config::defaults(BehaviorVersion::latest())
                    .region(Region::new(
                        env::var("AWS_REGION").unwrap_or_else(|_| "eu-central-1".to_string()),
                    ))
                    .load()
                    .await;
                Ok::<Client, anyhow::Error>(Client::new(&config))
            })
            .await
    }

    async fn load_secrets(&self) -> Result<Secrets> {
        let secret_name = CONSOLIDATED_SECRET_NAME.to_string();
        if let Some(secrets) = self.cache.get(&secret_name) {
            return Ok(secrets);
        }

        let client = self.client().await?;
        let resp = client
            .get_secret_value()
            .secret_id(CONSOLIDATED_SECRET_NAME)
            .send()
            .await
            .map_err(|e| anyhow!("AWS SecretsManager error: {e}"))?;

        let secret_string = resp
            .secret_string()
            .ok_or_else(|| anyhow!("Secret {} not found", CONSOLIDATED_SECRET_NAME))?;

        let secrets: Secrets = serde_json::from_str(secret_string)
            .map_err(|e| anyhow!("Failed to parse consolidated secret: {e}"))?;

        self.cache
            .insert(CONSOLIDATED_SECRET_NAME.to_string(), secrets.clone());
        Ok(secrets)
    }
}

#[async_trait]
impl SecretProvider for ConsolidatedAwsSecretProvider {
    async fn get(&self, key: SecretKey) -> Result<String> {
        if let Ok(val) = self.env.get(key).await {
            return Ok(val);
        }

        let secrets = self.load_secrets().await?;
        match key {
            SecretKey::DatabaseUrl => Ok(secrets.database_url),
            SecretKey::TaskApiKey => Ok(secrets.task_api_key),
            SecretKey::CaptchaSecret => Ok(secrets.captcha_secret),
            SecretKey::GoogleCreds => Ok(secrets.google_creds),
            SecretKey::EmailAccounts => Ok(secrets.email_accounts),
            SecretKey::SepaCreditorName => Ok(secrets.sepa_creditor_name),
            SecretKey::SepaCreditorIban => Ok(secrets.sepa_creditor_iban),
            SecretKey::SessionSecret => Ok(secrets.session_secret),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn env_secret_provider_returns_value_when_set() {
        let key = SecretKey::CaptchaSecret;
        unsafe {
            std::env::set_var(key.as_str(), "test-captcha-secret");
        }
        let provider = EnvSecretProvider;
        let result = provider.get(key).await;
        unsafe {
            std::env::remove_var(key.as_str());
        }
        assert_eq!(result.unwrap(), "test-captcha-secret");
    }
}
