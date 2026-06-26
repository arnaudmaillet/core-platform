//! The long-term WORM archive over an S3/MinIO bucket with **Object Lock in
//! compliance mode** — the durability backstop beyond the canonical ledger.
//!
//! Object Lock is configured on the bucket by ops (a retention period in
//! compliance mode), so once this adapter PUTs an object, not even the root
//! account can delete or overwrite it before the retention expires. This client
//! only ever writes; it never updates or deletes, mirroring the ledger's
//! append-only posture. Bytes flow store ⇄ this worker, never through the mesh.

use std::time::Duration as StdDuration;

use async_trait::async_trait;
use reqwest::header::CONTENT_TYPE;
use rusty_s3::{Bucket, Credentials, S3Action, UrlStyle};
use url::Url;

use crate::application::port::WormArchive;
use crate::domain::AuditRecord;
use crate::error::AuditError;

/// Connection settings for the WORM bucket. Built from env at the composition root
/// (Phase 5).
#[derive(Debug, Clone)]
pub struct ObjectLockConfig {
    pub endpoint: String,
    pub region: String,
    pub bucket: String,
    pub access_key: String,
    pub secret_key: String,
    pub presign_ttl: StdDuration,
    pub request_timeout: StdDuration,
}

pub struct ObjectLockArchive {
    bucket: Bucket,
    credentials: Credentials,
    http: reqwest::Client,
    presign_ttl: StdDuration,
}

impl ObjectLockArchive {
    pub fn new(config: ObjectLockConfig) -> Result<Self, AuditError> {
        let endpoint = Url::parse(&config.endpoint).map_err(|_| AuditError::ArchiveUnavailable)?;
        let bucket = Bucket::new(endpoint, UrlStyle::Path, config.bucket, config.region)
            .map_err(|_| AuditError::ArchiveUnavailable)?;
        let credentials = Credentials::new(config.access_key, config.secret_key);
        let http = reqwest::Client::builder()
            .timeout(config.request_timeout)
            .build()
            .map_err(|_| AuditError::ArchiveUnavailable)?;
        Ok(Self {
            bucket,
            credentials,
            http,
            presign_ttl: config.presign_ttl,
        })
    }

    /// Idempotently create the bucket (for local/test environments where it does
    /// not pre-exist; in production ops provisions it with Object Lock enabled). A
    /// 409 (already owned) is success.
    pub async fn ensure_bucket(&self) -> Result<(), AuditError> {
        let url = self.bucket.create_bucket(&self.credentials).sign(self.presign_ttl);
        let resp = self
            .http
            .put(url)
            .send()
            .await
            .map_err(|_| AuditError::ArchiveUnavailable)?;
        let status = resp.status();
        if status.is_success() || status == reqwest::StatusCode::CONFLICT {
            Ok(())
        } else {
            Err(AuditError::ArchiveUnavailable)
        }
    }

    async fn put(&self, key: &str, body: Vec<u8>) -> Result<(), AuditError> {
        let url: Url = self
            .bucket
            .put_object(Some(&self.credentials), key)
            .sign(self.presign_ttl);
        let resp = self
            .http
            .put(url)
            .header(CONTENT_TYPE, "application/json")
            .body(body)
            .send()
            .await
            .map_err(|_| AuditError::ArchiveUnavailable)?;
        if resp.status().is_success() {
            Ok(())
        } else {
            Err(AuditError::ArchiveUnavailable)
        }
    }
}

#[async_trait]
impl WormArchive for ObjectLockArchive {
    async fn archive(&self, record: &AuditRecord) -> Result<(), AuditError> {
        // One immutable object per record, addressed by its chain coordinates.
        let key = format!(
            "records/{}/{:020}-{}.json",
            record.partition().as_str(),
            record.sequence(),
            record.event().event_id().as_str()
        );
        let body = serde_json::to_vec(record).map_err(|_| AuditError::ArchiveUnavailable)?;
        self.put(&key, body).await
    }

    async fn store_export(
        &self,
        export_id: &str,
        content: &[u8],
    ) -> Result<String, AuditError> {
        let key = format!("exports/{export_id}.bundle");
        self.put(&key, content.to_vec()).await?;
        // An opaque, access-controlled reference the caller resolves out-of-band.
        Ok(format!("worm://{key}"))
    }
}
