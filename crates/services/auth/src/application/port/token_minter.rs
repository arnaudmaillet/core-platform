use async_trait::async_trait;

use crate::domain::value_object::{AccessTokenClaims, RefreshTokenHash};
use crate::error::AuthError;

/// A freshly generated opaque refresh token: the one-time plaintext shown to the
/// client and the hash to persist.
#[derive(Debug, Clone)]
pub struct GeneratedRefresh {
    /// 256-bit random handle — returned to the client exactly once.
    pub plaintext: String,
    /// Irreversible hash of `plaintext` — the only form stored.
    pub hash: RefreshTokenHash,
}

/// Token cryptography port (Phase 4 adapter: ES256 edge tokens + a CSPRNG for
/// refresh handles; PASETO v4 is a later drop-in).
///
/// Kept behind a port so the edge-token format is swappable without touching the
/// handlers, and so the signing key never reaches the application layer.
#[async_trait]
pub trait TokenMinter: Send + Sync + 'static {
    /// Serializes domain claims into a signed edge token string.
    async fn mint_access(&self, claims: &AccessTokenClaims) -> Result<String, AuthError>;

    /// Verifies a presented edge token and reconstructs its claims. Used by
    /// `Introspect`; returns [`AuthError::IdpTokenRejected`] on a bad signature /
    /// malformed token.
    async fn verify_access(&self, token: &str) -> Result<AccessTokenClaims, AuthError>;

    /// Generates a new opaque refresh token + its hash (pure CPU).
    fn generate_refresh(&self) -> Result<GeneratedRefresh, AuthError>;

    /// Recomputes the stored hash of a presented refresh plaintext, for lookup.
    fn hash_refresh(&self, plaintext: &str) -> Result<RefreshTokenHash, AuthError>;
}
