use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::AccountError;

/// Internal platform role assigned to an account.
///
/// Roles are cumulative and coarse-grained. Fine-grained authorisation (e.g.
/// per-resource write access) is handled by `permission_overrides` on the
/// aggregate. `privilege_level` provides a simple ordering for UI rendering
/// and audit trail prioritisation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountRole {
    /// Standard platform user.
    User,
    /// Can review and remove user-generated content.
    ContentModerator,
    /// Customer support access — can view account details.
    SupportAgent,
    /// Access to financial operations and ledger data.
    FinanceOperator,
    /// Full administrative access excluding super-admin operations.
    Admin,
    /// Unrestricted access including role assignment for all other roles.
    SuperAdmin,
}

impl AccountRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::User             => "user",
            Self::ContentModerator => "content_moderator",
            Self::SupportAgent     => "support_agent",
            Self::FinanceOperator  => "finance_operator",
            Self::Admin            => "admin",
            Self::SuperAdmin       => "super_admin",
        }
    }

    /// Numeric privilege level for ordering and audit comparisons.
    ///
    /// Higher values indicate broader access. Roles at the same level have
    /// different scope rather than different privilege depth.
    pub fn privilege_level(&self) -> u8 {
        match self {
            Self::User             => 1,
            Self::ContentModerator => 2,
            Self::SupportAgent     => 2,
            Self::FinanceOperator  => 3,
            Self::Admin            => 4,
            Self::SuperAdmin       => 5,
        }
    }
}

impl fmt::Display for AccountRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl TryFrom<&str> for AccountRole {
    type Error = AccountError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "user"              => Ok(Self::User),
            "content_moderator" => Ok(Self::ContentModerator),
            "support_agent"     => Ok(Self::SupportAgent),
            "finance_operator"  => Ok(Self::FinanceOperator),
            "admin"             => Ok(Self::Admin),
            "super_admin"       => Ok(Self::SuperAdmin),
            other => Err(AccountError::InvalidAccountRole(other.to_owned())),
        }
    }
}

impl TryFrom<String> for AccountRole {
    type Error = AccountError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::try_from(s.as_str())
    }
}
