use std::fmt;

use serde::{Deserialize, Serialize};

/// A normalized permission string, e.g. `"posts:write"` or `"ROLE_ADMIN"`.
///
/// This mirrors the `Permission` shape the `auth-context` library produces on the
/// inbound path, so the claims this service mints and the claims downstream
/// services read are the same vocabulary. Normalization from IdP-specific shapes
/// (Keycloak `realm_access.roles`, Okta `groups`, …) happens in the
/// infrastructure adapter; the domain only ever sees the normalized form.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Permission(String);

impl Permission {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Permission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_value() {
        assert_eq!(Permission::new("posts:write").as_str(), "posts:write");
    }
}
