// crates/account/src/domain/value_objects/trust_score.rs

use serde::{Deserialize, Serialize};
use shared_kernel::domain::value_objects::ValueObject;
use shared_kernel::errors::{DomainError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "i32", into = "i32")]
pub struct TrustScore(i32);

impl TrustScore {
    pub const MIN: i32 = 0;
    pub const MAX: i32 = 100;
    pub const DEFAULT: i32 = 100;

    // --- CONSTANTES DE RÈGLES MÉTIER ---
    pub const PENALTY_BAN: i32 = 100; // Un ban détruit le score
    pub const REWARD_UNBAN: i32 = 20; // Bonus de réhabilitation
    pub const REWARD_VERIFY: i32 = 10; // Bonus de vérification email/phone
    pub const REWARD_UNSUSPEND: i32 = 5; // Petit bonus après suspension
    pub const CRITICAL_THRESHOLD: i32 = 20; // Seuil sous lequel l'utilisateur est considéré "à risque"

    /// Création sécurisée avec validation
    pub fn try_new(value: i32) -> Result<Self> {
        let score = Self(value);
        score.validate()?;
        Ok(score)
    }

    /// Création par défaut (nouveaux comptes)
    pub fn new_perfect() -> Self {
        Self(Self::MAX)
    }

    /// Retourne la valeur brute pour SQLx ou les calculs
    pub fn value(&self) -> i32 {
        self.0
    }

    /// Permet de diminuer le score de manière sécurisée (ne descend jamais sous 0)
    pub fn penalize(&mut self, points: i32) {
        self.0 = (self.0 - points).max(Self::MIN);
    }

    /// Permet d'augmenter le score de manière sécurisée (ne dépasse jamais 100)
    pub fn reward(&mut self, points: i32) {
        self.0 = (self.0 + points).min(Self::MAX);
    }

    pub fn is_critical(&self) -> bool {
        self.0 <= Self::CRITICAL_THRESHOLD
    }
}

impl ValueObject for TrustScore {
    fn validate(&self) -> Result<()> {
        if self.0 < Self::MIN || self.0 > Self::MAX {
            return Err(DomainError::Validation {
                field: "trust_score",
                reason: format!(
                    "Trust score must be between {} and {}",
                    Self::MIN,
                    Self::MAX
                ),
            });
        }
        Ok(())
    }
}

// --- CONVERSIONS ---

impl TryFrom<i32> for TrustScore {
    type Error = DomainError;
    fn try_from(value: i32) -> Result<Self> {
        Self::try_new(value)
    }
}

impl From<TrustScore> for i32 {
    fn from(score: TrustScore) -> Self {
        score.0
    }
}
