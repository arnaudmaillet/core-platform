use std::sync::Arc;
use std::time::Duration as StdDuration;

use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};

use crate::application::port::{CdnGateway, ResolvedUrl};
use crate::domain::value_object::{DeliveryVisibility, StorageKey};
use crate::error::MediaError;

use crate::infrastructure::store::S3Client;

/// CDN gateway over a content-addressed origin. Public URLs are immutable (no
/// expiry — an edit is a new asset/hash/URL); signed URLs are minted per request
/// via the object store.
pub struct CloudFrontCdnGateway {
    /// CDN base URL, e.g. `https://cdn.example.com`.
    base_url: String,
    store: Arc<S3Client>,
    signed_ttl: Duration,
}

impl CloudFrontCdnGateway {
    pub fn new(base_url: String, store: Arc<S3Client>, signed_ttl: Duration) -> Self {
        Self { base_url, store, signed_ttl }
    }
}

#[async_trait]
impl CdnGateway for CloudFrontCdnGateway {
    async fn resolve(
        &self,
        key: &StorageKey,
        visibility: DeliveryVisibility,
        now: DateTime<Utc>,
    ) -> Result<ResolvedUrl, MediaError> {
        match visibility {
            DeliveryVisibility::Public => Ok(ResolvedUrl {
                url: format!("{}/{}", self.base_url.trim_end_matches('/'), key.as_str()),
                expires_at: None,
            }),
            DeliveryVisibility::Signed => {
                let ttl = self.signed_ttl.to_std().unwrap_or(StdDuration::from_secs(300));
                let url = self.store.presign_get(key.as_str(), ttl);
                Ok(ResolvedUrl {
                    url: url.to_string(),
                    expires_at: Some(now + self.signed_ttl),
                })
            }
        }
    }

    async fn invalidate(&self, keys: &[StorageKey]) -> Result<(), MediaError> {
        // Takedown-only path. A real CloudFront CreateInvalidation is a Phase-7
        // ops follow-up; content-addressed immutability means this never fires on
        // an edit, only on delete/quarantine.
        tracing::info!(count = keys.len(), "cdn invalidation requested (log stub)");
        Ok(())
    }
}
