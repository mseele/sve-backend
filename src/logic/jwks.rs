use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use jsonwebtoken::DecodingKey;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

#[cfg(test)]
use jsonwebtoken::EncodingKey;

const GOOGLE_JWKS_URL: &str = "https://www.googleapis.com/oauth2/v3/certs";
const JWKS_TTL: std::time::Duration = std::time::Duration::from_secs(24 * 3600);

#[async_trait]
pub(crate) trait JwksFetcher: Send + Sync {
    async fn fetch_certs(&self, kid: &str) -> Result<Arc<DecodingKey>>;
}

pub(crate) struct RealJwksFetcher {
    client: reqwest::Client,
    cache: Arc<RwLock<JwksCache>>,
}

impl RealJwksFetcher {
    pub(crate) fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            cache: Arc::new(RwLock::new(JwksCache::new())),
        }
    }
}

#[async_trait]
impl JwksFetcher for RealJwksFetcher {
    async fn fetch_certs(&self, kid: &str) -> Result<Arc<DecodingKey>> {
        let needs_refresh = {
            let cache = self.cache.read().await;
            cache.is_expired() || !cache.keys.contains_key(kid)
        };

        if needs_refresh {
            tracing::info!("Refreshing JWKS cache (expired or missing kid: {})", kid);
            let new_keys = fetch_jwks(&self.client, GOOGLE_JWKS_URL).await?;
            let mut cache = self.cache.write().await;
            cache.keys = new_keys;
            cache.last_updated = std::time::Instant::now();
        }

        let cache = self.cache.read().await;
        cache
            .keys
            .get(kid)
            .cloned()
            .ok_or_else(|| anyhow!("Unknown JWT key"))
    }
}

#[derive(Clone)]
struct JwksCache {
    keys: HashMap<String, Arc<DecodingKey>>,
    last_updated: std::time::Instant,
}

impl JwksCache {
    fn new() -> Self {
        Self {
            keys: HashMap::new(),
            last_updated: std::time::Instant::now(),
        }
    }

    fn is_expired(&self) -> bool {
        self.last_updated.elapsed() > JWKS_TTL
    }
}

async fn fetch_jwks(
    client: &reqwest::Client,
    jwks_url: &str,
) -> Result<HashMap<String, Arc<DecodingKey>>> {
    let res = client
        .get(jwks_url)
        .send()
        .await?
        .json::<HashMap<String, Vec<Jwk>>>()
        .await?;

    let mut keys = HashMap::new();
    if let Some(jwks_keys) = res.get("keys") {
        for key in jwks_keys {
            if let (Some(kid), Some(n), Some(e)) = (&key.kid, &key.n, &key.e) {
                let decoding_key = DecodingKey::from_rsa_components(n, e).unwrap();
                keys.insert(kid.clone(), Arc::new(decoding_key));
            }
        }
    }

    Ok(keys)
}

#[derive(Debug, Serialize, Deserialize)]
struct Jwk {
    kid: Option<String>,
    n: Option<String>,
    e: Option<String>,
}

#[cfg(test)]
pub(crate) struct FakeJwksFetcher {
    keys: RwLock<HashMap<String, Arc<DecodingKey>>>,
    error_on_next_call: RwLock<Option<String>>,
}

#[cfg(test)]
impl FakeJwksFetcher {
    pub(crate) fn new() -> Self {
        Self {
            keys: RwLock::new(HashMap::new()),
            error_on_next_call: RwLock::new(None),
        }
    }

    pub(crate) async fn add_key(&self, kid: &str, key: DecodingKey) {
        self.keys
            .write()
            .await
            .insert(kid.to_string(), Arc::new(key));
    }

    pub(crate) async fn set_next_call_error(&self, msg: &str) {
        *self.error_on_next_call.write().await = Some(msg.to_string());
    }
}

#[cfg(test)]
#[async_trait]
impl JwksFetcher for FakeJwksFetcher {
    async fn fetch_certs(&self, kid: &str) -> Result<Arc<DecodingKey>> {
        let err = self.error_on_next_call.write().await.take();
        if let Some(msg) = err {
            return Err(anyhow!("{}", msg));
        }

        self.keys
            .read()
            .await
            .get(kid)
            .cloned()
            .ok_or_else(|| anyhow!("Unknown kid: {kid}"))
    }
}

#[cfg(test)]
pub(crate) fn generate_test_rsa_key() -> (DecodingKey, EncodingKey) {
    use rsa::RsaPrivateKey;
    use rsa::pkcs1::{EncodeRsaPrivateKey, EncodeRsaPublicKey};

    let mut rng = rsa::rand_core::OsRng;
    let private_key = RsaPrivateKey::new(&mut rng, 2048).unwrap();
    let der = private_key.to_pkcs1_der().unwrap();
    let encoding_key = EncodingKey::from_rsa_der(der.as_bytes());
    let public_key = private_key.to_public_key();
    let decoding_key = DecodingKey::from_rsa_der(public_key.to_pkcs1_der().unwrap().as_bytes());
    (decoding_key, encoding_key)
}

#[cfg(test)]
mod tests {
    use super::*;

    use jsonwebtoken::{Algorithm, EncodingKey, Header};
    use serde::Serialize;

    #[derive(Debug, Serialize)]
    struct TestClaims {
        sub: String,
        exp: usize,
        iat: usize,
    }

    fn mint_test_token(encoding_key: &EncodingKey, kid: &str) -> String {
        let claims = TestClaims {
            sub: "test-user".to_string(),
            exp: 9999999999_usize,
            iat: 1000000000_usize,
        };
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some(kid.to_string());
        jsonwebtoken::encode(&header, &claims, encoding_key).unwrap()
    }

    #[tokio::test]
    async fn fake_fetcher_returns_key_when_added() {
        let (decoding_key, _encoding_key) = super::generate_test_rsa_key();
        let fetcher = FakeJwksFetcher::new();
        fetcher.add_key("test-kid", decoding_key).await;

        let result = fetcher.fetch_certs("test-kid").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn fake_fetcher_returns_error_when_configured() {
        let fetcher = FakeJwksFetcher::new();
        fetcher.set_next_call_error("simulated failure").await;

        let result = fetcher.fetch_certs("any-kid").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn fake_fetcher_returns_error_for_unknown_kid() {
        let fetcher = FakeJwksFetcher::new();
        let result = fetcher.fetch_certs("unknown-kid").await;
        assert!(result.is_err());
    }
}
