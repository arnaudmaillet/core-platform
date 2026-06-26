use std::time::Duration;

/// Connection settings for the S3-compatible object store. Built from env at the
/// composition root (Phase 5).
#[derive(Debug, Clone)]
pub struct S3Config {
    /// Endpoint URL (e.g. `https://s3.amazonaws.com` or a MinIO `http://…:9000`).
    pub endpoint: String,
    pub region: String,
    pub bucket: String,
    pub access_key: String,
    pub secret_key: String,
    /// Validity window for the server-side signed URLs this client mints.
    pub presign_ttl: Duration,
    /// Hard timeout on every object-store HTTP call (presign execution, GET/PUT,
    /// HEAD). Bounds the pipeline + delivery paths so a hung store can't wedge a
    /// worker — the media analogue of the Screen / query timeouts elsewhere.
    pub request_timeout: Duration,
}
