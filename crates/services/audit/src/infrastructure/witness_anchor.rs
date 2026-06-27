//! The production [`CheckpointAnchor`] (issue #483): a Merkle checkpoint root,
//! **signed in KMS's trust domain** and anchored to an **independent witness**.
//!
//! Why this exists: the v1 [`PgCheckpointAnchor`](super::PgCheckpointAnchor) stores
//! the anchor pointer in the *same* Postgres as the ledger, unsigned. A DB operator
//! who tampers a ledger row can rewrite that pointer too, and `verify_global` won't
//! catch them. Here the authority is an external copy the operator does not control:
//!
//! * **Sign** — the checkpoint's canonical bytes are signed by a [`KmsSigner`]
//!   (asymmetric KMS key, or the local HMAC fallback for dev), under a principal
//!   distinct from the ledger DB role.
//! * **Anchor** — the signed checkpoint is published to a [`Witness`]: a
//!   cross-account WORM bucket (S3 Object Lock, compliance mode). The Postgres
//!   pointer may remain as a *convenience index*, but the witness copy is the
//!   authority that `latest_anchored` reads back.
//! * **Verify** — `latest_anchored` validates the signature before returning the
//!   root; a forged witness entry (bad signature) is reported as
//!   `CheckpointVerificationFailed`. The verifier then reconciles the live heads
//!   against that root, so tampering both the ledger row *and* the Postgres pointer
//!   is still caught (the witness still holds the original signed root).

use std::sync::Arc;

use async_trait::async_trait;
use reqwest::header::CONTENT_TYPE;
use rusty_s3::actions::ListObjectsV2;
use rusty_s3::{Bucket, Credentials, S3Action, UrlStyle};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::application::port::CheckpointAnchor;
use crate::domain::MerkleCheckpoint;
use crate::error::AuditError;
use crate::infrastructure::kms::KmsSigner;
use crate::infrastructure::object_lock_archive::ObjectLockConfig;
use crate::infrastructure::pg_anchor::PgCheckpointAnchor;

/// A checkpoint plus the signature over its canonical bytes — the unit published to,
/// and read back from, the external witness.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedCheckpoint {
    pub checkpoint: MerkleCheckpoint,
    /// Base64 of the raw signature bytes (KMS or HMAC).
    pub signature_b64: String,
    /// The signing key id (informational; KMS recovers the key on verify).
    pub key_id: String,
    /// Signing algorithm tag, e.g. `ECDSA_SHA_256` or `HMAC_SHA_256` (dev).
    pub algorithm: String,
}

/// The canonical bytes signed over a checkpoint. Covers the whole checkpoint
/// (root + head count + timestamp), not just the root, so altering any field in a
/// witness entry invalidates the signature.
pub fn canonical_bytes(checkpoint: &MerkleCheckpoint) -> Vec<u8> {
    // The domain's serde representation is field-ordered and stable; good enough as
    // a canonical message for signing (we sign exactly what we verify).
    serde_json::to_vec(checkpoint).unwrap_or_default()
}

/// The independent witness the signed checkpoint is anchored to. The authority lives
/// here, in a trust domain the ledger DB operator does not control.
#[async_trait]
pub trait Witness: Send + Sync + 'static {
    /// Publish a signed checkpoint as an immutable witness entry.
    async fn publish(&self, signed: &SignedCheckpoint) -> Result<(), AuditError>;
    /// The most recently anchored signed checkpoint, if any.
    async fn latest(&self) -> Result<Option<SignedCheckpoint>, AuditError>;
}

/// The cross-account WORM-bucket witness (S3/MinIO Object Lock, compliance mode).
/// Each checkpoint is one immutable, timestamp-keyed object; "latest" is the
/// lexicographically greatest key under the prefix (keys are zero-padded millis).
pub struct ObjectLockWitness {
    bucket: Bucket,
    credentials: Credentials,
    http: reqwest::Client,
    presign_ttl: std::time::Duration,
}

const WITNESS_PREFIX: &str = "checkpoints/";

impl ObjectLockWitness {
    pub fn new(config: ObjectLockConfig) -> Result<Self, AuditError> {
        let endpoint = Url::parse(&config.endpoint).map_err(|_| AuditError::AnchorWitnessUnavailable)?;
        let bucket = Bucket::new(endpoint, UrlStyle::Path, config.bucket, config.region)
            .map_err(|_| AuditError::AnchorWitnessUnavailable)?;
        let credentials = Credentials::new(config.access_key, config.secret_key);
        let http = reqwest::Client::builder()
            .timeout(config.request_timeout)
            .build()
            .map_err(|_| AuditError::AnchorWitnessUnavailable)?;
        Ok(Self {
            bucket,
            credentials,
            http,
            presign_ttl: config.presign_ttl,
        })
    }

    /// Idempotently create the witness bucket (local/test; in production ops
    /// provisions a cross-account bucket with Object Lock enabled). 409 = success.
    pub async fn ensure_bucket(&self) -> Result<(), AuditError> {
        let url = self.bucket.create_bucket(&self.credentials).sign(self.presign_ttl);
        let resp = self
            .http
            .put(url)
            .send()
            .await
            .map_err(|_| AuditError::AnchorWitnessUnavailable)?;
        let status = resp.status();
        if status.is_success() || status == reqwest::StatusCode::CONFLICT {
            Ok(())
        } else {
            Err(AuditError::AnchorWitnessUnavailable)
        }
    }
}

#[async_trait]
impl Witness for ObjectLockWitness {
    async fn publish(&self, signed: &SignedCheckpoint) -> Result<(), AuditError> {
        // Zero-pad millis so lexical order == chronological order for the "latest"
        // scan; one immutable object per checkpoint.
        let key = format!(
            "{WITNESS_PREFIX}{:020}.json",
            signed.checkpoint.created_at().timestamp_millis()
        );
        let body = serde_json::to_vec(signed).map_err(|_| AuditError::AnchorWitnessUnavailable)?;
        let url: Url = self
            .bucket
            .put_object(Some(&self.credentials), &key)
            .sign(self.presign_ttl);
        let resp = self
            .http
            .put(url)
            .header(CONTENT_TYPE, "application/json")
            .body(body)
            .send()
            .await
            .map_err(|_| AuditError::AnchorWitnessUnavailable)?;
        if resp.status().is_success() {
            Ok(())
        } else {
            Err(AuditError::AnchorWitnessUnavailable)
        }
    }

    async fn latest(&self) -> Result<Option<SignedCheckpoint>, AuditError> {
        let mut list: ListObjectsV2 = self.bucket.list_objects_v2(Some(&self.credentials));
        list.with_prefix(WITNESS_PREFIX);
        let url = list.sign(self.presign_ttl);
        let resp = self
            .http
            .get(url)
            .send()
            .await
            .map_err(|_| AuditError::AnchorWitnessUnavailable)?;
        if !resp.status().is_success() {
            return Err(AuditError::AnchorWitnessUnavailable);
        }
        let body = resp.text().await.map_err(|_| AuditError::AnchorWitnessUnavailable)?;
        let parsed =
            ListObjectsV2::parse_response(&body).map_err(|_| AuditError::AnchorWitnessUnavailable)?;

        let Some(latest_key) = parsed.contents.into_iter().map(|c| c.key).max() else {
            return Ok(None);
        };

        let get_url = self
            .bucket
            .get_object(Some(&self.credentials), &latest_key)
            .sign(self.presign_ttl);
        let obj = self
            .http
            .get(get_url)
            .send()
            .await
            .map_err(|_| AuditError::AnchorWitnessUnavailable)?;
        if !obj.status().is_success() {
            return Err(AuditError::AnchorWitnessUnavailable);
        }
        let bytes = obj.bytes().await.map_err(|_| AuditError::AnchorWitnessUnavailable)?;
        let signed: SignedCheckpoint =
            serde_json::from_slice(&bytes).map_err(|_| AuditError::AnchorWitnessUnavailable)?;
        Ok(Some(signed))
    }
}

/// The production [`CheckpointAnchor`]: sign the root in KMS's domain, anchor it to
/// the witness, and (optionally) keep a Postgres convenience pointer. Reads come
/// from the witness — the authority a DB operator cannot rewrite.
pub struct WitnessCheckpointAnchor {
    signer: Arc<dyn KmsSigner>,
    signing_key_id: String,
    signing_algorithm: String,
    witness: Arc<dyn Witness>,
    /// Optional convenience index; never the authority. A failure here is logged,
    /// not fatal — the witness copy is what matters.
    pg_index: Option<PgCheckpointAnchor>,
}

impl WitnessCheckpointAnchor {
    pub fn new(
        signer: Arc<dyn KmsSigner>,
        signing_key_id: String,
        signing_algorithm: String,
        witness: Arc<dyn Witness>,
        pg_index: Option<PgCheckpointAnchor>,
    ) -> Self {
        Self {
            signer,
            signing_key_id,
            signing_algorithm,
            witness,
            pg_index,
        }
    }
}

#[async_trait]
impl CheckpointAnchor for WitnessCheckpointAnchor {
    async fn anchor(&self, checkpoint: &MerkleCheckpoint) -> Result<(), AuditError> {
        let message = canonical_bytes(checkpoint);
        let signature = self.signer.sign(&self.signing_key_id, &message).await?;
        let signed = SignedCheckpoint {
            checkpoint: checkpoint.clone(),
            signature_b64: base64::Engine::encode(
                &base64::engine::general_purpose::STANDARD,
                &signature,
            ),
            key_id: self.signing_key_id.clone(),
            algorithm: self.signing_algorithm.clone(),
        };

        self.witness.publish(&signed).await?;

        // Best-effort convenience pointer; the witness is the authority.
        if let Some(pg) = &self.pg_index
            && let Err(error) = pg.anchor(checkpoint).await
        {
            tracing::warn!(%error, "checkpoint witness anchored, but the Postgres convenience pointer failed");
        }
        Ok(())
    }

    async fn latest_anchored(&self) -> Result<Option<MerkleCheckpoint>, AuditError> {
        let Some(signed) = self.witness.latest().await? else {
            return Ok(None);
        };
        let signature = base64::Engine::decode(
            &base64::engine::general_purpose::STANDARD,
            &signed.signature_b64,
        )
        .map_err(|_| AuditError::CheckpointVerificationFailed)?;
        let message = canonical_bytes(&signed.checkpoint);

        // A witness entry whose signature does not validate is operator-level
        // tampering, not an availability fault — surface it as divergence.
        if !self.signer.verify(&signed.key_id, &message, &signature).await? {
            return Err(AuditError::CheckpointVerificationFailed);
        }
        Ok(Some(signed.checkpoint))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use chrono::{TimeZone, Utc};

    use super::*;
    use crate::domain::value_object::{PartitionKey, RecordHash};
    use crate::infrastructure::kms::LocalCheckpointSigner;

    /// An in-memory witness whose stored entry the test can *tamper* — standing in
    /// for an operator who somehow rewrote the external copy, to prove the signature
    /// check catches it.
    #[derive(Default)]
    struct InMemoryWitness {
        latest: Mutex<Option<SignedCheckpoint>>,
    }

    impl InMemoryWitness {
        fn tamper_root(&self) {
            let mut guard = self.latest.lock().unwrap();
            if let Some(signed) = guard.as_mut() {
                signed.checkpoint =
                    MerkleCheckpoint::over(&[head("forged", "deadbeef")], signed.checkpoint.created_at());
            }
        }
    }

    #[async_trait]
    impl Witness for InMemoryWitness {
        async fn publish(&self, signed: &SignedCheckpoint) -> Result<(), AuditError> {
            *self.latest.lock().unwrap() = Some(signed.clone());
            Ok(())
        }
        async fn latest(&self) -> Result<Option<SignedCheckpoint>, AuditError> {
            Ok(self.latest.lock().unwrap().clone())
        }
    }

    fn head(p: &str, h: &str) -> (PartitionKey, RecordHash) {
        (PartitionKey::new(p).unwrap(), RecordHash::digest(h.as_bytes()))
    }

    fn checkpoint() -> MerkleCheckpoint {
        let now = Utc.with_ymd_and_hms(2026, 6, 27, 9, 0, 0).unwrap();
        MerkleCheckpoint::over(&[head("p1", "x"), head("p2", "y")], now)
    }

    fn anchor(witness: Arc<dyn Witness>) -> WitnessCheckpointAnchor {
        WitnessCheckpointAnchor::new(
            Arc::new(LocalCheckpointSigner::new([5u8; 32])),
            "local-signing-key".to_owned(),
            "HMAC_SHA_256".to_owned(),
            witness,
            None,
        )
    }

    #[tokio::test]
    async fn signed_checkpoint_round_trips_through_the_witness() {
        let witness = Arc::new(InMemoryWitness::default());
        let a = anchor(Arc::clone(&witness) as Arc<dyn Witness>);
        let cp = checkpoint();
        a.anchor(&cp).await.unwrap();

        let read_back = a.latest_anchored().await.unwrap().unwrap();
        assert_eq!(read_back.root(), cp.root());
    }

    #[tokio::test]
    async fn nothing_anchored_yet_is_none() {
        let a = anchor(Arc::new(InMemoryWitness::default()));
        assert!(a.latest_anchored().await.unwrap().is_none());
    }

    /// The core #483 guarantee: a witness entry whose checkpoint was rewritten (its
    /// signature no longer matches) is reported as `CheckpointVerificationFailed`,
    /// not silently trusted.
    #[tokio::test]
    async fn a_tampered_witness_entry_fails_signature_verification() {
        let witness = Arc::new(InMemoryWitness::default());
        let a = anchor(Arc::clone(&witness) as Arc<dyn Witness>);
        a.anchor(&checkpoint()).await.unwrap();

        witness.tamper_root();

        let err = a.latest_anchored().await.unwrap_err();
        assert!(matches!(err, AuditError::CheckpointVerificationFailed));
    }
}
