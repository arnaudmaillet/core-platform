use std::time::Duration;

/// Connection settings for the S3-compatible object store. Built from env at the
/// composition root (Phase 5).
#[derive(Debug, Clone)]
pub struct S3Config {
    /// Endpoint URL for the server-side byte I/O this service performs itself
    /// (probe download, rendition upload, HEAD) — an in-network host the pods can
    /// reach, e.g. a MinIO `http://minio:9000` or `https://s3.amazonaws.com`.
    pub endpoint: String,
    /// Endpoint URL used to *sign the URLs handed back to clients* (the direct
    /// upload PUT and signed delivery GET). Must be resolvable from the caller's
    /// network, which the internal `endpoint` often is not: in the local fleet the
    /// pods reach MinIO at `http://minio:9000` while a device signs against
    /// `http://localhost:9000`. In prod both are usually the public S3/CDN host.
    pub public_endpoint: String,
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
