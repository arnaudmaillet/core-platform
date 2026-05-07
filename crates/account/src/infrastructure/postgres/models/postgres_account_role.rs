use serde::Deserialize;
use sqlx::Type;

use crate::domain::value_objects::AccountRole;

/// Représentation technique du rôle pour PostgreSQL
#[derive(Debug, Deserialize, Clone, Type)]
#[sqlx(type_name = "TEXT")]
pub enum PostgresAccountRole {
    USER,
    MODERATOR,
    STAFF,
    ADMIN,
}

// --- CONVERSIONS ---

/// DB -> DOMAINE (Lecture)
impl From<PostgresAccountRole> for AccountRole {
    fn from(sql_role: PostgresAccountRole) -> Self {
        match sql_role {
            PostgresAccountRole::USER => AccountRole::USER,
            PostgresAccountRole::MODERATOR => AccountRole::MODERATOR,
            PostgresAccountRole::STAFF => AccountRole::STAFF,
            PostgresAccountRole::ADMIN => AccountRole::ADMIN,
        }
    }
}

/// DOMAINE -> DB (Écriture)
impl From<AccountRole> for PostgresAccountRole {
    fn from(domain_role: AccountRole) -> Self {
        match domain_role {
            AccountRole::USER => PostgresAccountRole::USER,
            AccountRole::MODERATOR => PostgresAccountRole::MODERATOR,
            AccountRole::STAFF => PostgresAccountRole::STAFF,
            AccountRole::ADMIN => PostgresAccountRole::ADMIN,
        }
    }
}
