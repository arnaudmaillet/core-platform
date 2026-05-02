use crate::domain::claims::Claims;
use shared_kernel::domain::value_objects::JwtToken;
use thiserror::Error;

#[derive(Error, Debug, PartialEq, Eq)]
pub enum AuthError {
    #[error("Invalid token signature or structure")]
    InvalidToken,

    #[error("Identity provider connection failed")]
    DiscoveryFailed,

    #[error("Token expired")]
    Expired,
}

pub trait TokenValidator: Send + Sync {
    fn validate(&self, token: &JwtToken) -> Result<Claims, AuthError>;
}
