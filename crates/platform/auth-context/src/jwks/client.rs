use std::collections::HashMap;
use std::time::Duration;

use jsonwebtoken::DecodingKey;
use reqwest::Client;
use serde::Deserialize;

use crate::AuthError;

// ── Wire types ───────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct JwksResponse {
    keys: Vec<JwkKey>,
}

#[derive(Deserialize)]
struct JwkKey {
    kid: String,
    kty: String,
    // RSA public key components (base64url-encoded, no padding)
    n: Option<String>,
    e: Option<String>,
    // EC public key components
    crv: Option<String>,
    x: Option<String>,
    y: Option<String>,
}

// ── Public API ───────────────────────────────────────────────────────────────

/// Stateless HTTP client that fetches a JWKS document and converts each key
/// into a [`DecodingKey`] ready for use by [`JwtDecoder`].
///
/// A single `JwksClient` instance is shared by the [`JwksRefresher`] background
/// task across its entire lifetime. It carries no mutable state between fetches.
///
/// [`JwtDecoder`]: crate::JwtDecoder
/// [`JwksRefresher`]: crate::JwksRefresher
pub struct JwksClient {
    http: Client,
    url: String,
}

impl JwksClient {
    /// Constructs a client targeting `url` with the given per-request `timeout`.
    ///
    /// The underlying `reqwest::Client` is built once and reused across all
    /// fetches, sharing the connection pool.
    ///
    /// # Panics
    ///
    /// Panics if the TLS backend cannot be initialised (extremely unlikely in
    /// a correctly linked binary).
    pub fn new(url: impl Into<String>, timeout: Duration) -> Self {
        let http = Client::builder()
            .timeout(timeout)
            .https_only(false) // allow http:// in local/test deployments
            .build()
            .expect("failed to build JWKS HTTP client — TLS backend unavailable");

        Self {
            http,
            url: url.into(),
        }
    }

    /// Fetches the JWKS document and returns a map of `kid → DecodingKey`.
    ///
    /// Keys whose `kty` is unsupported (e.g. `oct`, `OKP`) are skipped with a
    /// `WARN` log rather than causing the entire fetch to fail. This tolerates
    /// mixed-type JWKS responses from providers that publish both signing and
    /// encryption keys in the same set.
    ///
    /// # Errors
    ///
    /// Returns [`AuthError::JwksUnavailable`] on any HTTP or parse failure.
    pub async fn fetch(&self) -> Result<HashMap<String, DecodingKey>, AuthError> {
        let response = self
            .http
            .get(&self.url)
            .send()
            .await
            .map_err(|e| AuthError::JwksUnavailable(e.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            return Err(AuthError::JwksUnavailable(format!(
                "JWKS endpoint returned HTTP {status}"
            )));
        }

        let body: JwksResponse = response
            .json()
            .await
            .map_err(|e| AuthError::JwksUnavailable(format!("JWKS parse error: {e}")))?;

        let mut keys: HashMap<String, DecodingKey> = HashMap::with_capacity(body.keys.len());

        for jwk in body.keys {
            match Self::build_decoding_key(&jwk) {
                Ok(key) => {
                    tracing::debug!(kid = %jwk.kid, kty = %jwk.kty, "JWKS key loaded");
                    keys.insert(jwk.kid, key);
                }
                Err(reason) => {
                    tracing::warn!(kid = %jwk.kid, kty = %jwk.kty, %reason, "skipping undecodable JWKS key");
                }
            }
        }

        Ok(keys)
    }

    fn build_decoding_key(jwk: &JwkKey) -> Result<DecodingKey, String> {
        match jwk.kty.as_str() {
            "RSA" => {
                let n = jwk.n.as_deref().ok_or("RSA key missing 'n'")?;
                let e = jwk.e.as_deref().ok_or("RSA key missing 'e'")?;
                DecodingKey::from_rsa_components(n, e)
                    .map_err(|e| format!("RSA key construction failed: {e}"))
            }

            "EC" => {
                let _crv = jwk.crv.as_deref().unwrap_or("P-256");
                let x = jwk.x.as_deref().ok_or("EC key missing 'x'")?;
                let y = jwk.y.as_deref().ok_or("EC key missing 'y'")?;
                DecodingKey::from_ec_components(x, y)
                    .map_err(|e| format!("EC key construction failed: {e}"))
            }

            kty => Err(format!("unsupported key type: {kty}")),
        }
    }
}
