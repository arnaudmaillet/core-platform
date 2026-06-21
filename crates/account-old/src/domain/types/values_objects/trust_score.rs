// crates/account/src/domain/types/trust_score.rs

use serde::{Deserialize, Serialize};
use shared_kernel::core::{Error, Result, ValueObject};

use crate::domain::types::TrustAmount; // 💡 ALIGNEMENT : Importation de ton nouveau VO

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "i32", into = "i32")]
pub struct TrustScore(i32);

impl TrustScore {
    pub const MIN: i32 = 0;
    pub const MAX: i32 = 100;
    pub const DEFAULT: i32 = 100;

    // --- CONSTANTES DE RÈGLES MÉTIER ---
    pub const CRITICAL_THRESHOLD: i32 = 20;

    /// Création sécurisée avec validation
    pub fn try_new(value: i32) -> Result<Self> {
        let score = Self(value);
        score.validate()?;
        Ok(score)
    }

    /// Création par défaut (nouveaux comptes)
    pub fn new_max() -> Self {
        Self(Self::MAX)
    }

    pub fn new_min() -> Self {
        Self(Self::MIN)
    }

    pub fn from_raw(value: i32) -> Self {
        Self(value)
    }

    /// Retourne la valeur brute pour SQLx ou les calculs
    pub fn value(&self) -> i32 {
        self.0
    }

    /// Permet de diminuer le score de manière sécurisée (ne descend jamais sous 0)
    pub fn penalize(&mut self, amount: TrustAmount) {
        // 💡 PLUS DE .abs() ! Un cast propre en u32 et l'utilisation de saturating_sub
        // garantit qu'on ne subit aucun overflow ou valeur négative incohérente.
        let current = self.0 as u32;
        let new_score = current.saturating_sub(amount.value());

        self.0 = (new_score as i32).max(Self::MIN);
    }

    /// Permet d'augmenter le score de manière sécurisée (ne dépasse jamais 100)
    pub fn reward(&mut self, amount: TrustAmount) {
        let current = self.0 as u32;
        let new_score = current.saturating_add(amount.value());

        self.0 = (new_score as i32).min(Self::MAX);
    }

    pub fn is_critical(&self) -> bool {
        self.0 <= Self::CRITICAL_THRESHOLD
    }
}

impl ValueObject for TrustScore {
    fn validate(&self) -> Result<()> {
        if self.0 < Self::MIN || self.0 > Self::MAX {
            return Err(Error::validation(
                "trust_score",
                format!(
                    "Trust score must be between {} and {}",
                    Self::MIN,
                    Self::MAX
                ),
            ));
        }
        Ok(())
    }
}

// --- CONVERSIONS ---

impl TryFrom<i32> for TrustScore {
    type Error = Error;
    fn try_from(value: i32) -> Result<Self> {
        Self::try_new(value)
    }
}

impl From<TrustScore> for i32 {
    fn from(score: TrustScore) -> Self {
        score.0
    }
}

impl Default for TrustScore {
    fn default() -> Self {
        Self::new_max()
    }
}
