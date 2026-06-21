use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::AccountError;

/// The external IdP subject claim (`sub`) that uniquely identifies the
/// authenticated identity behind this account.
///
/// Auth0 subs look like `"google-oauth2|123456"`, Keycloak subs are UUID
/// strings. The value is kept opaque so both layouts work without coercion.
/// It is immutable after account creation.
#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct IdentityId(String);

impl IdentityId {
    const MAX_LEN: usize = 255;

    pub fn new(value: impl Into<String>) -> Result<Self, AccountError> {
        let s = value.into();
        if s.is_empty() {
            return Err(AccountError::InvalidIdentityId(
                "identity_id must not be empty".into(),
            ));
        }
        if s.len() > Self::MAX_LEN {
            return Err(AccountError::InvalidIdentityId(format!(
                "identity_id exceeds maximum length of {} characters",
                Self::MAX_LEN
            )));
        }
        Ok(Self(s))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

impl fmt::Display for IdentityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
