use crate::{AuthError, CurrentPrincipal};

/// Strategy for transforming raw JWT claims into a [`CurrentPrincipal`].
///
/// Implementations are injected into [`JwtDecoder`] at construction time.
/// The decoder calls [`extract`] once per verified token — after the
/// cryptographic signature check has already passed — so implementations can
/// assume the claims are structurally valid and not tampered with.
///
/// # Implementing for a custom provider
///
/// ```rust,ignore
/// use auth_context::{ClaimsExtractor, CurrentPrincipal, AuthError, Permission, PrincipalId};
/// use serde::Deserialize;
///
/// #[derive(Deserialize)]
/// struct MyClaims { sub: String, roles: Vec<String> }
///
/// struct MyExtractor;
///
/// impl ClaimsExtractor<MyClaims> for MyExtractor {
///     fn extract(&self, raw: MyClaims) -> Result<CurrentPrincipal<MyClaims>, AuthError> {
///         Ok(CurrentPrincipal {
///             user_id: PrincipalId::new(&raw.sub),
///             tenant_id: None,
///             permissions: raw.roles.iter().map(Permission::new).collect(),
///             raw_claims: raw,
///         })
///     }
/// }
/// ```
///
/// [`JwtDecoder`]: crate::JwtDecoder
pub trait ClaimsExtractor<C>: Send + Sync + 'static {
    /// Maps the provider-specific claim set `C` into a [`CurrentPrincipal<C>`].
    ///
    /// # Errors
    ///
    /// Return [`AuthError::ClaimsExtractionFailed`] when a required claim is
    /// absent, has an unexpected type, or cannot be mapped to the platform model.
    fn extract(&self, raw: C) -> Result<CurrentPrincipal<C>, AuthError>;
}
