use std::time::Duration;

use reqwest::header::{CONTENT_LENGTH, CONTENT_RANGE, CONTENT_TYPE, RANGE};
use reqwest::StatusCode;
use rusty_s3::{Bucket, Credentials, S3Action, UrlStyle};
use url::Url;

use super::config::S3Config;
use crate::error::MediaError;

/// A thin S3-compatible client: presigns URLs (for the client's direct upload and
/// for signed delivery) and performs the server-side byte I/O the pipeline needs.
/// Bytes only ever flow store ⇄ this worker — never through the gRPC/Kafka mesh.
pub struct S3Client {
    bucket: Bucket,
    credentials: Credentials,
    http: reqwest::Client,
    presign_ttl: Duration,
}

impl S3Client {
    pub fn new(config: S3Config) -> Result<Self, MediaError> {
        let endpoint = Url::parse(&config.endpoint).map_err(|e| MediaError::PresignFailed {
            reason: format!("invalid object-store endpoint: {e}"),
        })?;
        let bucket = Bucket::new(endpoint, UrlStyle::Path, config.bucket, config.region).map_err(
            |e| MediaError::PresignFailed { reason: format!("invalid bucket: {e}") },
        )?;
        let credentials = Credentials::new(config.access_key, config.secret_key);
        // The HTTP client carries the hard request timeout: a stuck object-store
        // call elapses into `ObjectStoreTimeout` (retryable) rather than hanging.
        let http = reqwest::Client::builder()
            .timeout(config.request_timeout)
            .build()
            .map_err(|e| MediaError::PresignFailed {
                reason: format!("failed to build the object-store HTTP client: {e}"),
            })?;
        Ok(Self { bucket, credentials, http, presign_ttl: config.presign_ttl })
    }

    /// Presigned PUT URL the client uploads to directly.
    pub fn presign_put(&self, key: &str, ttl: Duration) -> Url {
        self.bucket.put_object(Some(&self.credentials), key).sign(ttl)
    }

    /// Presigned GET URL (server-side reads + signed delivery).
    pub fn presign_get(&self, key: &str, ttl: Duration) -> Url {
        self.bucket.get_object(Some(&self.credentials), key).sign(ttl)
    }

    fn presign_delete(&self, key: &str, ttl: Duration) -> Url {
        self.bucket.delete_object(Some(&self.credentials), key).sign(ttl)
    }

    /// Server-side download of an object's bytes (for probe / derive).
    pub async fn get_bytes(&self, key: &str) -> Result<Vec<u8>, MediaError> {
        let url = self.presign_get(key, self.presign_ttl);
        let resp = self.http.get(url).send().await.map_err(reqwest_err)?;
        if resp.status() == StatusCode::NOT_FOUND {
            return Err(MediaError::ObjectNotFound { key: key.to_owned() });
        }
        if !resp.status().is_success() {
            return Err(MediaError::ObjectStoreUnavailable);
        }
        Ok(resp.bytes().await.map_err(reqwest_err)?.to_vec())
    }

    /// Server-side upload of a derivative object.
    pub async fn put_bytes(
        &self,
        key: &str,
        bytes: Vec<u8>,
        content_type: &str,
    ) -> Result<(), MediaError> {
        let url = self.presign_put(key, self.presign_ttl);
        let resp = self
            .http
            .put(url)
            .header(CONTENT_TYPE, content_type)
            .body(bytes)
            .send()
            .await
            .map_err(reqwest_err)?;
        if !resp.status().is_success() {
            return Err(MediaError::ObjectStoreUnavailable);
        }
        Ok(())
    }

    /// Deletes an object. Idempotent: a 404 is treated as success.
    pub async fn delete(&self, key: &str) -> Result<(), MediaError> {
        let url = self.presign_delete(key, self.presign_ttl);
        let resp = self.http.delete(url).send().await.map_err(reqwest_err)?;
        let status = resp.status();
        if status.is_success() || status == StatusCode::NOT_FOUND {
            Ok(())
        } else {
            Err(MediaError::ObjectStoreUnavailable)
        }
    }

    /// Returns an object's size, or `None` if it does not exist. Implemented as a
    /// ranged GET (`bytes=0-0`) so it works without a presigned HEAD action.
    pub async fn object_size(&self, key: &str) -> Result<Option<u64>, MediaError> {
        let url = self.presign_get(key, self.presign_ttl);
        let resp = self
            .http
            .get(url)
            .header(RANGE, "bytes=0-0")
            .send()
            .await
            .map_err(reqwest_err)?;
        let status = resp.status();
        if status == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !(status.is_success() || status == StatusCode::PARTIAL_CONTENT) {
            return Err(MediaError::ObjectStoreUnavailable);
        }
        // Prefer the total from `Content-Range: bytes 0-0/<total>`, else
        // `Content-Length`.
        let size = resp
            .headers()
            .get(CONTENT_RANGE)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.rsplit('/').next().map(str::to_owned))
            .and_then(|total| total.parse::<u64>().ok())
            .or_else(|| {
                resp.headers()
                    .get(CONTENT_LENGTH)
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.parse::<u64>().ok())
            })
            .unwrap_or(0);
        Ok(Some(size))
    }

    pub fn presign_ttl(&self) -> Duration {
        self.presign_ttl
    }

    /// Liveness probe: a cheap reachability check against the bucket. A missing
    /// sentinel object (404) is healthy; only a transport/5xx fault is unhealthy.
    pub async fn health(&self) -> Result<(), MediaError> {
        self.object_size("__healthcheck__").await.map(|_| ())
    }

    /// Idempotently creates the bucket (for local/test environments where it does
    /// not pre-exist). A 409 (already owned) is treated as success.
    pub async fn ensure_bucket(&self) -> Result<(), MediaError> {
        let url = self.bucket.create_bucket(&self.credentials).sign(self.presign_ttl);
        let resp = self.http.put(url).send().await.map_err(reqwest_err)?;
        let status = resp.status();
        if status.is_success() || status == StatusCode::CONFLICT {
            Ok(())
        } else {
            Err(MediaError::ObjectStoreUnavailable)
        }
    }
}

fn reqwest_err(e: reqwest::Error) -> MediaError {
    if e.is_timeout() {
        MediaError::ObjectStoreTimeout
    } else {
        MediaError::ObjectStoreUnavailable
    }
}
