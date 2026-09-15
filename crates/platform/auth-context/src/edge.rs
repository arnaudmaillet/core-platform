//! Helpers for the platform's own **edge tokens** — the short-lived ES256 JWTs
//! `auth` mints (ADR-0005) and every service verifies in-process.
//!
//! The wire layout is fixed by `crates/services/auth`'s minter:
//!
//! | claim   | meaning                                              |
//! |---------|------------------------------------------------------|
//! | `sub`   | the internal **account** id (never the IdP subject)  |
//! | `sid`   | the session the token belongs to                     |
//! | `gen`   | the revocation generation                            |
//! | `perms` | normalised permissions ([`EDGE_PERMISSIONS_CLAIM`])  |
//! | `pids`  | the **profile** ids the account owns ([`EDGE_PROFILES_CLAIM`]) |
//!
//! `pids` exists because the client-facing surface is keyed by profile id while
//! the token subject is an account id (one account owns N profiles): a service
//! validating "may this caller act as profile P" checks `P ∈ pids` without a
//! lookup, keeping verification stateless as the ADR requires.

use std::sync::Arc;

use jsonwebtoken::Algorithm;

use crate::{
    AuthContextConfig, JwksCache, JwksClient, JwksRefresher, JwtDecoder, OidcClaims,
    OidcClaimsExtractor,
};

/// The claim carrying the profile ids the token's account owns (a JSON array of
/// UUID strings). Absent or empty ⇒ the caller may act as no profile.
pub const EDGE_PROFILES_CLAIM: &str = "pids";

/// The claim carrying the session id.
pub const EDGE_SESSION_CLAIM: &str = "sid";

/// The decoder specialisation every edge-token verifier in the fleet uses.
pub type EdgeDecoder = JwtDecoder<OidcClaims, OidcClaimsExtractor>;

/// Builds the edge-token decoder and starts its JWKS refresher.
///
/// The refresher task is detached (dropping the handle keeps it running for the
/// process lifetime) and shares its cache with the decoder, so key rotations are
/// picked up transparently. A cold start does not require the JWKS endpoint to
/// be reachable: verification fails closed (unknown `kid`) until the first
/// successful fetch.
///
/// The edge token is ES256; RS256 is accepted too in case the JWKS ever mixes
/// key types (mirrors the realtime gateway and the audit gate).
pub fn spawn_edge_decoder(config: &AuthContextConfig) -> Arc<EdgeDecoder> {
    let cache = JwksCache::new();
    let client = JwksClient::new(config.jwks_url.clone(), config.fetch_timeout);
    let _refresher =
        JwksRefresher::spawn(client, cache.clone(), config.refresh_interval, config.max_backoff);
    Arc::new(JwtDecoder::with_algorithms(
        config,
        cache,
        OidcClaimsExtractor::platform_edge(),
        vec![Algorithm::ES256, Algorithm::RS256],
    ))
}

/// The profile ids carried by a verified edge token's [`EDGE_PROFILES_CLAIM`].
pub fn profile_ids(claims: &OidcClaims) -> impl Iterator<Item = &str> {
    claims
        .extra
        .get(EDGE_PROFILES_CLAIM)
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str())
}

/// The session id carried by a verified edge token's [`EDGE_SESSION_CLAIM`].
pub fn session_id(claims: &OidcClaims) -> Option<&str> {
    claims
        .extra
        .get(EDGE_SESSION_CLAIM)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn claims(extra: serde_json::Value) -> OidcClaims {
        serde_json::from_value(json!({
            "sub": "acct-1",
            "exp": 4_102_444_800_i64,
        }))
        .map(|mut c: OidcClaims| {
            if let serde_json::Value::Object(map) = extra {
                c.extra.extend(map);
            }
            c
        })
        .unwrap()
    }

    #[test]
    fn profile_ids_reads_the_pids_array() {
        let c = claims(json!({ "pids": ["p-1", "p-2", 42] }));
        let ids: Vec<&str> = profile_ids(&c).collect();
        assert_eq!(ids, vec!["p-1", "p-2"]); // non-strings are skipped
    }

    #[test]
    fn profile_ids_is_empty_when_the_claim_is_absent_or_malformed() {
        assert_eq!(profile_ids(&claims(json!({}))).count(), 0);
        assert_eq!(profile_ids(&claims(json!({ "pids": "p-1" }))).count(), 0);
    }

    #[test]
    fn session_id_reads_sid_and_ignores_empty() {
        assert_eq!(session_id(&claims(json!({ "sid": "s-1" }))), Some("s-1"));
        assert_eq!(session_id(&claims(json!({ "sid": "" }))), None);
        assert_eq!(session_id(&claims(json!({}))), None);
    }
}
