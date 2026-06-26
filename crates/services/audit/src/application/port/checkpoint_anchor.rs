use async_trait::async_trait;

use crate::domain::MerkleCheckpoint;
use crate::error::AuditError;

/// The bridge to the independent witness that makes operator-level tampering
/// detectable. A periodic Merkle checkpoint over the partition heads is signed (in
/// the separate KMS trust domain) and anchored here — to an RFC 3161 timestamp
/// authority and/or a cross-account WORM bucket. The verifier later reconciles the
/// live chains against the latest anchored root.
///
/// An unreachable witness is `AUD-2005 AnchorWitnessUnavailable` (retryable):
/// chaining continues; only the anchoring is deferred.
#[async_trait]
pub trait CheckpointAnchor: Send + Sync + 'static {
    /// Sign and publish a checkpoint to the external witness.
    async fn anchor(&self, checkpoint: &MerkleCheckpoint) -> Result<(), AuditError>;

    /// The most recently anchored checkpoint, if any — the trusted root the
    /// global integrity check reconciles the live heads against.
    async fn latest_anchored(&self) -> Result<Option<MerkleCheckpoint>, AuditError>;
}
