use async_trait::async_trait;
use fred::interfaces::KeysInterface;
use redis_storage::RedisClient;
use serde::Deserialize;

use crate::application::port::{ContentHash, CorpusMatch, ScreenCorpus};
use crate::domain::value_object::PolicyCategory;
use crate::error::ModerationError;

use super::keys::corpus_key;

/// The on-the-wire shape of a corpus entry: `mod:corpus:{algo}:value` → this JSON.
/// The corpus is populated out-of-band by the hash-ingestion pipeline; this
/// adapter only reads it.
#[derive(Debug, Deserialize)]
struct StoredMatch {
    categories: Vec<String>,
    reference: String,
}

/// Redis-backed [`ScreenCorpus`] — the bounded-latency, deterministic hash lookup
/// behind the Plane C gate. A connectivity failure surfaces as
/// [`ModerationError::HashCorpusUnavailable`], which the caller's per-category fail
/// policy turns into a hard block for catastrophic categories.
#[derive(Clone)]
pub struct RedisScreenCorpus {
    client: RedisClient,
}

impl RedisScreenCorpus {
    pub fn new(client: RedisClient) -> Self {
        Self { client }
    }
}

fn corpus_err(_e: fred::error::Error) -> ModerationError {
    // A corpus lookup failure is treated as the corpus being unavailable, so the
    // fail-closed caller blocks rather than admitting unscreened content.
    ModerationError::HashCorpusUnavailable
}

fn parse_entry(raw: &str) -> Result<CorpusMatch, ModerationError> {
    let stored: StoredMatch = serde_json::from_str(raw).map_err(|e| ModerationError::SignalRejected {
        reason: format!("malformed corpus entry: {e}"),
    })?;
    let categories = stored
        .categories
        .iter()
        .map(|c| PolicyCategory::try_from(c.as_str()))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(CorpusMatch { categories, reference: stored.reference })
}

#[async_trait]
impl ScreenCorpus for RedisScreenCorpus {
    async fn screen(
        &self,
        hashes: &[ContentHash],
        _text: Option<&str>,
        _categories: &[PolicyCategory],
    ) -> Result<Option<CorpusMatch>, ModerationError> {
        // Concurrency note: hashes are few (one per algorithm); a sequential set of
        // single-key GETs keeps each lookup slot-local on the cluster.
        for h in hashes {
            let raw: Option<String> = self
                .client
                .get(corpus_key(&h.algorithm, &h.value))
                .await
                .map_err(corpus_err)?;
            if let Some(raw) = raw {
                return Ok(Some(parse_entry(&raw)?));
            }
        }
        Ok(None)
    }
}
