use crate::domain::value_objects::AccountState;

/// Il permet de mapper l'ENUM PostgreSQL sans polluer le Domaine avec SQLx.
#[derive(Debug, sqlx::Type)]
#[sqlx(type_name = "account_state", rename_all = "lowercase")]
pub enum PostgresAccountState {
    Pending,
    Active,
    Deactivated,
    Suspended,
    Banned,
}

// --- CONVERSIONS ---

/// Convertit le type SQLx vers le Domaine (Lecture depuis la DB)
impl From<PostgresAccountState> for AccountState {
    fn from(sql_status: PostgresAccountState) -> Self {
        match sql_status {
            PostgresAccountState::Pending => Self::Pending,
            PostgresAccountState::Active => Self::Active,
            PostgresAccountState::Deactivated => Self::Deactivated,
            PostgresAccountState::Suspended => Self::Suspended,
            PostgresAccountState::Banned => Self::Banned,
        }
    }
}

/// Convertit le Domaine vers le type SQLx (Écriture vers la DB)
impl From<AccountState> for PostgresAccountState {
    fn from(domain_status: AccountState) -> Self {
        match domain_status {
            AccountState::Pending => PostgresAccountState::Pending,
            AccountState::Active => PostgresAccountState::Active,
            AccountState::Deactivated => PostgresAccountState::Deactivated,
            AccountState::Suspended => PostgresAccountState::Suspended,
            AccountState::Banned => PostgresAccountState::Banned,
        }
    }
}
