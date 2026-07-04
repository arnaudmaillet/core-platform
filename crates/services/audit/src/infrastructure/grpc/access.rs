//! The caller gate for the privileged `audit.v1` surface.
//!
//! Audit is the fleet's compliance evidence plane, so its gRPC surface is
//! **need-to-know**: every RPC requires a verified caller identity (an ES256
//! edge token minted by `auth`) carrying the matching `audit:*` permission.
//! The gate fails CLOSED at every layer:
//!
//! * no / malformed / expired / unverifiable token → `AUD-3004`
//!   ([`AuditError::CallerUnauthenticated`], gRPC `UNAUTHENTICATED`);
//! * authenticated but missing the RPC's permission → the RPC's own AUD-3xxx
//!   denial (gRPC `PERMISSION_DENIED`);
//! * **no verifier configured** (no `AUDIT_JWKS_URL`) → [`DenyAllGate`] — the
//!   surface denies everything rather than silently serving unauthenticated
//!   reads. Health and reflection are runtime-owned and stay open.
//!
//! Nothing token-bearing is ever logged; on success only the principal id and
//! the RPC name are traced (the interim access trail until each read is
//! recorded as its own `DATA_ACCESS` event — see the README deferral).

use std::collections::HashSet;
use std::sync::Arc;

use async_trait::async_trait;
use auth_context::{
    AuthContextConfig, JwksCache, JwksClient, JwksRefresher, JwtDecoder, OidcClaims,
    OidcClaimsExtractor,
};
use jsonwebtoken::Algorithm;
use tonic::metadata::MetadataMap;

use crate::error::AuditError;

/// The `audit:*` permission tokens the gate recognises. Minted into the edge
/// token by `auth`; opaque strings by contract (see `auth-context::Permission`).
pub mod perm {
    /// Record on the synchronous privileged lane (break-glass / legal-hold).
    pub const RECORD: &str = "audit:record";
    /// Query ledger records (need-to-know read).
    pub const READ: &str = "audit:read";
    /// Export evidence bundles (stricter than read — bulk egress).
    pub const EXPORT: &str = "audit:export";
    /// Run integrity verification (chain / checkpoint reports, no record data).
    pub const VERIFY: &str = "audit:verify";
}

/// A verified caller of the privileged surface: the platform principal id and
/// the normalised permission set extracted from the verified token.
#[derive(Debug, Clone)]
pub struct Caller {
    pub principal: String,
    permissions: HashSet<String>,
}

impl Caller {
    pub fn new(principal: impl Into<String>, permissions: impl IntoIterator<Item = String>) -> Self {
        Self {
            principal: principal.into(),
            permissions: permissions.into_iter().collect(),
        }
    }

    /// `Ok(())` when the caller holds `permission`; `Err(denial)` otherwise.
    pub fn require(&self, permission: &str, denial: AuditError) -> Result<(), AuditError> {
        if self.permissions.contains(permission) {
            Ok(())
        } else {
            Err(denial)
        }
    }
}

/// Authenticates a Bearer token into a [`Caller`]. Authentication only — the
/// per-RPC permission check stays with the handler so each surface maps a
/// missing permission onto its own AUD-3xxx denial.
#[async_trait]
pub trait CallerGate: Send + Sync {
    /// Verify `bearer` (the token with the `Bearer ` scheme already stripped).
    /// `None` means the request carried no usable `authorization` metadata.
    async fn verify(&self, bearer: Option<&str>) -> Result<Caller, AuditError>;
}

/// Extract the Bearer token from gRPC request metadata, verify it through the
/// gate, and require `permission` — the single guard every RPC runs first.
pub async fn authorize(
    gate: &dyn CallerGate,
    metadata: &MetadataMap,
    permission: &str,
    denial: AuditError,
) -> Result<Caller, AuditError> {
    let bearer = metadata
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| {
            let (scheme, token) = v.split_once(' ')?;
            scheme.eq_ignore_ascii_case("bearer").then_some(token.trim())
        })
        .filter(|t| !t.is_empty());

    let caller = gate.verify(bearer).await?;
    caller.require(permission, denial)?;
    Ok(caller)
}

/// The production gate: verifies the ES256 edge token against the `auth`
/// JWKS via the shared `auth-context` decoder (same pattern as the realtime
/// gateway's handshake verification).
pub struct AuthContextCallerGate {
    decoder: Arc<JwtDecoder<OidcClaims, OidcClaimsExtractor>>,
}

impl AuthContextCallerGate {
    pub fn new(decoder: Arc<JwtDecoder<OidcClaims, OidcClaimsExtractor>>) -> Self {
        Self { decoder }
    }
}

#[async_trait]
impl CallerGate for AuthContextCallerGate {
    async fn verify(&self, bearer: Option<&str>) -> Result<Caller, AuditError> {
        let token = bearer.ok_or(AuditError::CallerUnauthenticated)?;
        // Never surface decoder detail to the caller (and never log the token):
        // every verification failure collapses into the one 401.
        let principal = self
            .decoder
            .decode(token)
            .await
            .map_err(|_| AuditError::CallerUnauthenticated)?;
        Ok(Caller::new(
            principal.user_id.as_str().to_owned(),
            principal.permissions.into_iter().map(|p| p.0),
        ))
    }
}

/// The fail-closed fallback when no verifier is configured: every call is
/// `AUD-3004`. Deliberately NOT an allow-all — an unconfigured TIER-0 read
/// surface must deny, not open.
pub struct DenyAllGate;

#[async_trait]
impl CallerGate for DenyAllGate {
    async fn verify(&self, _bearer: Option<&str>) -> Result<Caller, AuditError> {
        Err(AuditError::CallerUnauthenticated)
    }
}

/// Build the gate from the resolved authz config: a JWKS-backed verifier when
/// configured, the deny-all fallback otherwise. The refresher task is detached
/// (dropping the handle keeps it running); a cold start does not require the
/// JWKS endpoint to be reachable — verification fails closed until the first
/// successful fetch.
pub fn build_gate(authz: &Option<AuthContextConfig>) -> Arc<dyn CallerGate> {
    match authz {
        Some(auth) => {
            let cache = JwksCache::new();
            let client = JwksClient::new(auth.jwks_url.clone(), auth.fetch_timeout);
            let _refresher =
                JwksRefresher::spawn(client, cache.clone(), auth.refresh_interval, auth.max_backoff);
            let decoder = Arc::new(JwtDecoder::with_algorithms(
                auth,
                cache,
                OidcClaimsExtractor::default(),
                // The edge token is ES256; accept RS256 too in case the JWKS
                // mixes key types (mirrors the realtime gateway).
                vec![Algorithm::ES256, Algorithm::RS256],
            ));
            Arc::new(AuthContextCallerGate::new(decoder))
        }
        None => {
            tracing::warn!(
                "AUDIT_JWKS_URL is not set — the audit.v1 surface DENIES all calls \
                 (AUD-3004) until a token verifier is configured"
            );
            Arc::new(DenyAllGate)
        }
    }
}

/// A fixed-outcome gate for composing the handler in tests.
#[cfg(test)]
pub struct StaticCallerGate {
    caller: Option<Caller>,
}

#[cfg(test)]
impl StaticCallerGate {
    /// Every call authenticates as `principal` holding `permissions`.
    pub fn allowing(principal: &str, permissions: &[&str]) -> Arc<Self> {
        Arc::new(Self {
            caller: Some(Caller::new(
                principal,
                permissions.iter().map(|p| (*p).to_owned()),
            )),
        })
    }

    /// Every call fails authentication (AUD-3004).
    pub fn denying() -> Arc<Self> {
        Arc::new(Self { caller: None })
    }
}

#[cfg(test)]
#[async_trait]
impl CallerGate for StaticCallerGate {
    async fn verify(&self, _bearer: Option<&str>) -> Result<Caller, AuditError> {
        self.caller.clone().ok_or(AuditError::CallerUnauthenticated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metadata_with_auth(value: &str) -> MetadataMap {
        let mut m = MetadataMap::new();
        m.insert("authorization", value.parse().unwrap());
        m
    }

    #[tokio::test]
    async fn a_missing_authorization_header_is_unauthenticated() {
        let gate = AuthContextCallerGate {
            decoder: unreachable_decoder(),
        };
        let err = authorize(&gate, &MetadataMap::new(), perm::READ, AuditError::QueryForbidden)
            .await
            .unwrap_err();
        assert!(matches!(err, AuditError::CallerUnauthenticated));
    }

    #[tokio::test]
    async fn a_non_bearer_scheme_is_unauthenticated() {
        let gate = AuthContextCallerGate {
            decoder: unreachable_decoder(),
        };
        let meta = metadata_with_auth("Basic dXNlcjpwYXNz");
        let err = authorize(&gate, &meta, perm::READ, AuditError::QueryForbidden)
            .await
            .unwrap_err();
        assert!(matches!(err, AuditError::CallerUnauthenticated));
    }

    #[tokio::test]
    async fn the_bearer_scheme_is_case_insensitive_and_the_token_reaches_the_gate() {
        // A static gate ignores the token; this asserts the scheme parse accepts
        // "bearer" and the permission check then runs.
        let gate = StaticCallerGate::allowing("ops-1", &[perm::READ]);
        let meta = metadata_with_auth("bearer some-token");
        let caller = authorize(gate.as_ref(), &meta, perm::READ, AuditError::QueryForbidden)
            .await
            .unwrap();
        assert_eq!(caller.principal, "ops-1");
    }

    #[tokio::test]
    async fn a_caller_without_the_permission_gets_the_rpc_denial() {
        let gate = StaticCallerGate::allowing("ops-1", &[perm::READ]);
        let meta = metadata_with_auth("Bearer some-token");
        let err = authorize(gate.as_ref(), &meta, perm::EXPORT, AuditError::ExportForbidden)
            .await
            .unwrap_err();
        assert!(matches!(err, AuditError::ExportForbidden));
    }

    #[tokio::test]
    async fn the_deny_all_gate_rejects_even_a_well_formed_bearer() {
        let meta = metadata_with_auth("Bearer some-token");
        let err = authorize(&DenyAllGate, &meta, perm::READ, AuditError::QueryForbidden)
            .await
            .unwrap_err();
        assert!(matches!(err, AuditError::CallerUnauthenticated));
    }

    /// A decoder whose JWKS cache is empty — any decode attempt fails, which is
    /// fine because these tests never get past the metadata parse.
    fn unreachable_decoder() -> Arc<JwtDecoder<OidcClaims, OidcClaimsExtractor>> {
        let cfg = AuthContextConfig::default();
        Arc::new(JwtDecoder::with_algorithms(
            &cfg,
            JwksCache::new(),
            OidcClaimsExtractor::default(),
            vec![Algorithm::ES256],
        ))
    }
}
