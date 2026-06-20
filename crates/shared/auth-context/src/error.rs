use thiserror::Error;

/// All failure modes that can be returned by the auth-context layer.
///
/// The variants are intentionally coarse enough to be safe to surface to
/// callers (no internal implementation details leak), but granular enough
/// for structured log filtering and operational alerting.
#[derive(Debug, Error)]
pub enum AuthError {
    /// The JWT cryptographic signature does not match the public key.
    #[error("JWT signature is invalid")]
    InvalidSignature,

    /// The `exp` claim is in the past (beyond the configured clock-skew leeway).
    #[error("JWT has expired")]
    TokenExpired,

    /// The `nbf` claim is in the future (beyond the configured clock-skew leeway).
    #[error("JWT is not yet valid (nbf constraint violated)")]
    TokenNotYetValid,

    /// The JWT header is present but does not contain a `kid` field.
    ///
    /// Both the OIDC spec and the JWKS key-lookup algorithm require `kid`.
    /// Tokens without it cannot be verified without trying every cached key.
    #[error("JWT header is missing the required 'kid' field")]
    MissingKid,

    /// The `kid` from the JWT header has no matching key in the local JWKS cache.
    ///
    /// This may indicate key rotation in progress. The refresher will pick up
    /// the new key on the next scheduled cycle.
    #[error("no JWKS key found for kid '{0}'")]
    UnknownKid(String),

    /// The JWKS endpoint returned an error or was unreachable.
    ///
    /// The enclosed message contains the underlying HTTP or parsing error detail,
    /// safe for operational logs but not for API error responses.
    #[error("JWKS endpoint is unavailable: {0}")]
    JwksUnavailable(String),

    /// The token string is structurally invalid (not three base64url segments,
    /// unsupported algorithm declared in the header, etc.).
    #[error("JWT is malformed: {0}")]
    MalformedToken(String),

    /// The `aud` claim does not contain the expected audience value.
    #[error("JWT audience validation failed")]
    InvalidAudience,

    /// The `iss` claim does not match the expected issuer URL.
    #[error("JWT issuer validation failed")]
    InvalidIssuer,

    /// The [`ClaimsExtractor`] rejected the decoded claims.
    ///
    /// [`ClaimsExtractor`]: crate::ClaimsExtractor
    #[error("claims extraction failed: {0}")]
    ClaimsExtractionFailed(String),
}
