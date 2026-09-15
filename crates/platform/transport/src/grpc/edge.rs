//! The **client edge**: the authenticated, allow-listed gRPC listener a service
//! exposes to the public load balancer.
//!
//! A fleet service has two listeners (see `service-runtime`):
//!
//! * the **mesh** listener (`<SVC>_GRPC_ADDR`) — in-cluster callers only, guarded by
//!   NetworkPolicy, no token check (today's posture, unchanged);
//! * the **edge** listener (`GRPC_EDGE_ADDR`) — what the ALB targets. Every request
//!   goes through [`EdgeLayer`](crate::grpc::layer::EdgeLayer): the method must be
//!   declared in the service's [`EdgePolicy`] (anything else is `UNIMPLEMENTED`),
//!   and unless the rule is [`EdgeAccess::Public`] a valid ES256 edge token must be
//!   presented. The verified caller is then attached to the request as an
//!   [`EdgePrincipal`] extension (and as the `auth-context` task-local principal).
//!
//! # Identity comes from the token, not the request
//!
//! Client-facing contracts carry the acting identity as a request field
//! (`profile_id`, `sender_id`, `owner_id`, …). On the edge that field is a *claim*
//! the caller makes; the handler must bind it to the verified token with
//! [`require_account`] (the field is an account id: must equal `sub`) or
//! [`require_profile`] (the field is a profile id: must be one the account owns,
//! i.e. in the `pids` claim). On the mesh listener there is no principal and both
//! helpers are no-ops, so in-cluster callers keep working unchanged.

use std::sync::Arc;

use auth_context::{edge, CurrentPrincipal, OidcClaims};
use tonic::Status;

/// What a request to an edge-exposed method must present.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeAccess {
    /// No token required. Reserved for the RPCs that *produce* a session (login,
    /// refresh); still rate-limited per method.
    Public,
    /// A valid edge token is required; the verified principal is attached.
    Authenticated,
    /// A valid edge token carrying this permission (an `auth` `perms` entry) is
    /// required.
    Permission(&'static str),
}

/// One edge-exposed RPC: the full gRPC method path (`/<package.Service>/<Method>`)
/// and the access it requires.
#[derive(Debug, Clone, Copy)]
pub struct EdgeRule {
    pub method: &'static str,
    pub access: EdgeAccess,
}

/// A service's edge allow-list. Methods absent from it are **not exposed** on the
/// edge listener — internal RPCs, staff consoles and anything not yet reviewed stay
/// mesh-only by construction.
pub type EdgePolicy = &'static [EdgeRule];

/// An [`EdgeAccess::Public`] rule.
pub const fn public(method: &'static str) -> EdgeRule {
    EdgeRule { method, access: EdgeAccess::Public }
}

/// An [`EdgeAccess::Authenticated`] rule.
pub const fn authenticated(method: &'static str) -> EdgeRule {
    EdgeRule { method, access: EdgeAccess::Authenticated }
}

/// An [`EdgeAccess::Permission`] rule.
pub const fn permission(method: &'static str, permission: &'static str) -> EdgeRule {
    EdgeRule { method, access: EdgeAccess::Permission(permission) }
}

/// Validates an [`EdgePolicy`]: every method is a `/<service>/<method>` path and
/// no method is declared twice. Run at boot so a typo fails the pod, not a client.
pub fn validate_policy(policy: &[EdgeRule]) -> Result<(), String> {
    let mut seen = std::collections::HashSet::with_capacity(policy.len());
    for rule in policy {
        let m = rule.method;
        let well_formed = m.starts_with('/')
            && m.matches('/').count() == 2
            && m.split('/').skip(1).all(|part| !part.is_empty() && !part.contains(char::is_whitespace));
        if !well_formed {
            return Err(format!("edge policy: `{m}` is not a `/<package.Service>/<Method>` path"));
        }
        if !seen.insert(m) {
            return Err(format!("edge policy: `{m}` is declared twice"));
        }
    }
    Ok(())
}

/// The verified caller of an edge request. Attached by the edge layer as a request
/// extension; read it with [`principal`].
#[derive(Clone)]
pub struct EdgePrincipal(Arc<CurrentPrincipal<OidcClaims>>);

impl EdgePrincipal {
    pub fn new(principal: Arc<CurrentPrincipal<OidcClaims>>) -> Self {
        Self(principal)
    }

    /// The token subject — the caller's **account** id.
    pub fn account_id(&self) -> &str {
        self.0.user_id.as_str()
    }

    /// The caller's session id (`sid` claim), when the token carries one.
    pub fn session_id(&self) -> Option<&str> {
        edge::session_id(&self.0.raw_claims)
    }

    /// The profile ids the caller's account owns (`pids` claim).
    pub fn profile_ids(&self) -> impl Iterator<Item = &str> {
        edge::profile_ids(&self.0.raw_claims)
    }

    /// `true` when `profile_id` is one of the caller's profiles.
    pub fn owns_profile(&self, profile_id: &str) -> bool {
        self.profile_ids().any(|p| p == profile_id)
    }

    /// `true` when the token carries `permission`.
    pub fn has_permission(&self, permission: &str) -> bool {
        self.0.has_permission(permission)
    }

    /// The underlying verified principal (raw claims included).
    pub fn inner(&self) -> &CurrentPrincipal<OidcClaims> {
        &self.0
    }

    pub fn into_inner(self) -> Arc<CurrentPrincipal<OidcClaims>> {
        self.0
    }
}

impl std::fmt::Debug for EdgePrincipal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EdgePrincipal")
            .field("account_id", &self.account_id())
            .finish_non_exhaustive()
    }
}

/// The verified edge caller, or `None` when the request came in over the mesh
/// listener (or hit a [`EdgeAccess::Public`] method).
pub fn principal<T>(request: &tonic::Request<T>) -> Option<&EdgePrincipal> {
    request.extensions().get::<EdgePrincipal>()
}

/// Binds a request's **account**-id actor field to the verified caller: on the
/// edge the field must equal the token subject; over the mesh (no principal) it
/// is accepted as-is.
pub fn require_account<T>(request: &tonic::Request<T>, account_id: &str) -> Result<(), Status> {
    match principal(request) {
        None => Ok(()),
        Some(p) if p.account_id() == account_id => Ok(()),
        Some(_) => Err(Status::permission_denied(
            "the authenticated account may not act as the requested account",
        )),
    }
}

/// Binds a request's **profile**-id actor field to the verified caller: on the
/// edge the profile must be one the token's account owns (`pids`); over the mesh
/// (no principal) it is accepted as-is.
pub fn require_profile<T>(request: &tonic::Request<T>, profile_id: &str) -> Result<(), Status> {
    match principal(request) {
        None => Ok(()),
        Some(p) if p.owns_profile(profile_id) => Ok(()),
        Some(_) => Err(Status::permission_denied(
            "the authenticated account may not act as the requested profile",
        )),
    }
}

/// Requires the verified caller to carry `permission`. Over the mesh (no
/// principal) it is a no-op, like the actor helpers.
pub fn require_permission<T>(request: &tonic::Request<T>, permission: &str) -> Result<(), Status> {
    match principal(request) {
        None => Ok(()),
        Some(p) if p.has_permission(permission) => Ok(()),
        Some(_) => Err(Status::permission_denied("missing permission")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use auth_context::{Permission, PrincipalId};
    use serde_json::json;

    fn principal_with(pids: &[&str], perms: &[&str]) -> EdgePrincipal {
        let mut raw: OidcClaims =
            serde_json::from_value(json!({ "sub": "acct-1", "exp": 4_102_444_800_i64, "sid": "s-1" }))
                .unwrap();
        raw.extra.insert("pids".into(), json!(pids));
        EdgePrincipal::new(Arc::new(CurrentPrincipal {
            user_id: PrincipalId::new("acct-1"),
            tenant_id: None,
            permissions: perms.iter().map(|p| Permission::new(*p)).collect(),
            raw_claims: raw,
        }))
    }

    fn edge_request(p: EdgePrincipal) -> tonic::Request<()> {
        let mut req = tonic::Request::new(());
        req.extensions_mut().insert(p);
        req
    }

    #[test]
    fn validate_accepts_well_formed_unique_paths() {
        validate_policy(&[
            authenticated("/post.v1.PostService/CreatePost"),
            public("/auth.v1.AuthService/Login"),
        ])
        .unwrap();
    }

    #[test]
    fn validate_rejects_malformed_and_duplicate_paths() {
        assert!(validate_policy(&[authenticated("post.v1.PostService/CreatePost")]).is_err());
        assert!(validate_policy(&[authenticated("/post.v1.PostService")]).is_err());
        assert!(validate_policy(&[authenticated("/a/b/c")]).is_err());
        assert!(validate_policy(&[authenticated("/a/b"), public("/a/b")]).is_err());
    }

    #[test]
    fn mesh_requests_have_no_principal_and_pass_every_check() {
        let req = tonic::Request::new(());
        assert!(principal(&req).is_none());
        require_account(&req, "anyone").unwrap();
        require_profile(&req, "anyone").unwrap();
        require_permission(&req, "anything").unwrap();
    }

    #[test]
    fn require_account_binds_to_the_token_subject() {
        let req = edge_request(principal_with(&[], &[]));
        require_account(&req, "acct-1").unwrap();
        let err = require_account(&req, "acct-2").unwrap_err();
        assert_eq!(err.code(), tonic::Code::PermissionDenied);
    }

    #[test]
    fn require_profile_binds_to_the_owned_profiles() {
        let req = edge_request(principal_with(&["p-1", "p-2"], &[]));
        require_profile(&req, "p-2").unwrap();
        let err = require_profile(&req, "p-9").unwrap_err();
        assert_eq!(err.code(), tonic::Code::PermissionDenied);
        // The account id is not a profile id.
        assert!(require_profile(&req, "acct-1").is_err());
    }

    #[test]
    fn require_permission_checks_the_perms_claim() {
        let req = edge_request(principal_with(&[], &["audit:read"]));
        require_permission(&req, "audit:read").unwrap();
        assert!(require_permission(&req, "audit:export").is_err());
    }

    #[test]
    fn principal_exposes_session_and_profiles() {
        let p = principal_with(&["p-1"], &[]);
        assert_eq!(p.account_id(), "acct-1");
        assert_eq!(p.session_id(), Some("s-1"));
        assert_eq!(p.profile_ids().collect::<Vec<_>>(), vec!["p-1"]);
    }
}
