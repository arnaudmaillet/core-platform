use serde::Deserialize;
use sqlx::Type;

use crate::domain::value_objects::AccountRole;

/// Représentation technique du rôle pour PostgreSQL
#[derive(Debug, Deserialize, Clone, Type)]
#[sqlx(type_name = "internal_role", rename_all = "lowercase")]
pub enum PostgresAccountRole {
    User,
    Moderator,
    Staff,
    Admin,
}

// --- CONVERSIONS ---

/// DB -> DOMAINE (Lecture)
impl From<PostgresAccountRole> for AccountRole {
    fn from(sql_role: PostgresAccountRole) -> Self {
        match sql_role {
            PostgresAccountRole::User => AccountRole::User,
            PostgresAccountRole::Moderator => AccountRole::Moderator,
            PostgresAccountRole::Staff => AccountRole::Staff,
            PostgresAccountRole::Admin => AccountRole::Admin,
        }
    }
}

/// DOMAINE -> DB (Écriture)
impl From<AccountRole> for PostgresAccountRole {
    fn from(domain_role: AccountRole) -> Self {
        match domain_role {
            AccountRole::User => PostgresAccountRole::User,
            AccountRole::Moderator => PostgresAccountRole::Moderator,
            AccountRole::Staff => PostgresAccountRole::Staff,
            AccountRole::Admin => PostgresAccountRole::Admin,
        }
    }
}
