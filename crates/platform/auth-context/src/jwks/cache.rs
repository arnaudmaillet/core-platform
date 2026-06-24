use std::collections::HashMap;
use std::sync::Arc;

use jsonwebtoken::DecodingKey;
use tokio::sync::RwLock;

/// Thread-safe, in-memory store of JWKS decoding keys, keyed by `kid`.
///
/// ## Concurrency model
///
/// - **Reads** (`get`) acquire a shared `tokio::sync::RwLock` read guard.
///   Hundreds of concurrent JWT decoders can call this simultaneously without
///   contending against each other — they only contend against the single
///   periodic writer.
/// - **Writes** (`replace`) hold the write lock only for the duration of the
///   `HashMap` pointer swap (nanoseconds), not for the network fetch itself.
///   The [`JwksRefresher`] fetches the new key set over HTTP *before* acquiring
///   the write lock, minimising the write-lock hold time.
///
/// ## Key rotation
///
/// `replace` does a full swap. During the brief window between a write finishing
/// and the next read, a request arriving with the old `kid` will receive
/// [`crate::AuthError::UnknownKid`]. This is the correct behaviour: if a key
/// was rotated away at the IdP, it should no longer be trusted.
///
/// [`JwksRefresher`]: crate::JwksRefresher
#[derive(Clone)]
pub struct JwksCache {
    inner: Arc<RwLock<HashMap<String, DecodingKey>>>,
}

impl JwksCache {
    /// Creates an empty cache. The first successful JWKS fetch populates it.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Looks up the [`DecodingKey`] for `kid`.
    ///
    /// Returns a clone of the stored key so the read lock is not held by
    /// the caller during subsequent JWT verification work.
    pub async fn get(&self, kid: &str) -> Option<DecodingKey> {
        self.inner.read().await.get(kid).cloned()
    }

    /// Atomically replaces the entire key set with `keys`.
    ///
    /// The write lock is held only for the pointer swap itself.
    /// Called exclusively by [`JwksRefresher`] after a successful JWKS fetch.
    ///
    /// [`JwksRefresher`]: crate::JwksRefresher
    pub async fn replace(&self, keys: HashMap<String, DecodingKey>) {
        *self.inner.write().await = keys;
    }

    /// Returns the number of keys currently in the cache.
    pub async fn len(&self) -> usize {
        self.inner.read().await.len()
    }

    /// Returns `true` if the cache holds no keys.
    ///
    /// A `true` result at request time means the first JWKS fetch has not
    /// completed yet — the refresher logs this condition at startup.
    pub async fn is_empty(&self) -> bool {
        self.inner.read().await.is_empty()
    }
}

impl Default for JwksCache {
    fn default() -> Self {
        Self::new()
    }
}
