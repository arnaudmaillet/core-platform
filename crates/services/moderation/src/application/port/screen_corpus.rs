use async_trait::async_trait;

use crate::domain::value_object::PolicyCategory;
use crate::error::ModerationError;

/// A precomputed content fingerprint supplied by the caller (e.g. `media`). The
/// screen path never receives raw bytes — only these hashes and, optionally, short
/// text for the critical-term blocklist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentHash {
    /// e.g. `"pdq"`, `"md5"`, `"photodna"`.
    pub algorithm: String,
    pub value: String,
}

/// A positive hit against the known-bad corpus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorpusMatch {
    /// The catastrophic-harm categories the content matched.
    pub categories: Vec<PolicyCategory>,
    /// Corpus entry reference, recorded on the decision for the audit trail.
    pub reference: String,
}

/// The **Screen corpus** (Plane C) — the deterministic, bounded-latency lookup
/// behind the pre-publish gate: perceptual/cryptographic hash matching plus a
/// critical-term blocklist. No ML inference. The adapter (Redis/bloom, Phase 4)
/// must be fast and is fail-closed for catastrophic categories: an error here is
/// surfaced as [`ModerationError::ScreenUnavailable`]/`HashCorpusUnavailable`, and
/// the *caller's* policy converts that into a hard block.
#[async_trait]
pub trait ScreenCorpus: Send + Sync + 'static {
    /// Screens the given hashes (and optional text) against the corpus for the
    /// requested categories. Returns `Some(match)` on a known-bad hit, `None`
    /// when nothing matched (which means "no known-bad match", not "approved").
    async fn screen(
        &self,
        hashes: &[ContentHash],
        text: Option<&str>,
        categories: &[PolicyCategory],
    ) -> Result<Option<CorpusMatch>, ModerationError>;
}
