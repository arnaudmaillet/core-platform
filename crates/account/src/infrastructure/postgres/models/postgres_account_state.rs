use crate::domain::value_objects::AccountState;

/// Il permet de mapper l'ENUM PostgreSQL sans polluer le Domaine avec SQLx.
#[derive(Debug, sqlx::Type)]
#[sqlx(type_name = "account_state")]
pub enum PostgresAccountState {
    PENDING,
    ACTIVE,
    DEACTIVATED,
    SUSPENDED,
    BANNED,
}

// --- CONVERSIONS ---

/// Convertit le type SQLx vers le Domaine (Lecture depuis la DB)
impl From<PostgresAccountState> for AccountState {
    fn from(sql_status: PostgresAccountState) -> Self {
        match sql_status {
            PostgresAccountState::PENDING => Self::PENDING,
            PostgresAccountState::ACTIVE => Self::ACTIVE,
            PostgresAccountState::DEACTIVATED => Self::DEACTIVATED,
            PostgresAccountState::SUSPENDED => Self::SUSPENDED,
            PostgresAccountState::BANNED => Self::BANNED,
        }
    }
}

/// Convertit le Domaine vers le type SQLx (Écriture vers la DB)
impl From<&AccountState> for PostgresAccountState {
    fn from(domain_status: &AccountState) -> Self {
        match domain_status {
            AccountState::PENDING => PostgresAccountState::PENDING,
            AccountState::ACTIVE => PostgresAccountState::ACTIVE,
            AccountState::DEACTIVATED => PostgresAccountState::DEACTIVATED,
            AccountState::SUSPENDED => PostgresAccountState::SUSPENDED,
            AccountState::BANNED => PostgresAccountState::BANNED,
        }
    }
}
