use crate::domain::types::BetaTier;
use infra_sqlx::sqlx::Type;

/// Mappe l'ENUM PostgreSQL `beta_tier` vers une structure compatible SQLx.
#[derive(Debug, Type)]
#[sqlx(type_name = "TEXT")]
pub enum PostgresBetaTier {
    NONE,
    BETA,
    ALPHA,
    INTERNAL,
}

// --- CONVERSIONS ---

/// Lecture depuis la DB : Postgres -> Domaine
impl From<PostgresBetaTier> for BetaTier {
    fn from(sql_tier: PostgresBetaTier) -> Self {
        match sql_tier {
            PostgresBetaTier::NONE => Self::NONE,
            PostgresBetaTier::BETA => Self::BETA,
            PostgresBetaTier::ALPHA => Self::ALPHA,
            PostgresBetaTier::INTERNAL => Self::INTERNAL,
        }
    }
}

/// Écriture vers la DB : Domaine -> Postgres
impl From<BetaTier> for PostgresBetaTier {
    fn from(domain_tier: BetaTier) -> Self {
        match domain_tier {
            BetaTier::NONE => PostgresBetaTier::NONE,
            BetaTier::BETA => PostgresBetaTier::BETA,
            BetaTier::ALPHA => PostgresBetaTier::ALPHA,
            BetaTier::INTERNAL => PostgresBetaTier::INTERNAL,
        }
    }
}
