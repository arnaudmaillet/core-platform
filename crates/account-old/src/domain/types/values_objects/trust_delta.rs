// crates/account/src/domain/types/trust_delta.rs

use serde::{Deserialize, Serialize};
use std::ops::Neg;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TrustDelta(i32);

impl TrustDelta {
    pub const PENALTY_BAN: TrustDelta = TrustDelta::from_raw(100); // Un ban détruit le score
    pub const REWARD_UNBAN: TrustDelta = TrustDelta::from_raw(20); // Bonus de réhabilitation
    pub const REWARD_UNSUSPEND: TrustDelta = TrustDelta::from_raw(5); // Petit bonus après suspension

    pub const fn from_raw(value: i32) -> Self {
        Self(value)
    }

    /// Garantit que le delta est traité comme une valeur positive (magnitude).
    /// Utile pour les fonctions comme .penalize(amount)
    pub fn abs(&self) -> i32 {
        self.0.abs()
    }

    /// Retourne la valeur brute (peut être négative si utilisé pour un changement relatif)
    pub fn value(&self) -> i32 {
        self.0
    }

    /// Inverse le delta (utile pour transformer une pénalité en ajustement négatif)
    pub fn negate(self) -> Self {
        Self(-self.0)
    }
}

// --- ERGONOMIE & CONVERSIONS ---

impl From<i32> for TrustDelta {
    fn from(value: i32) -> Self {
        Self::from_raw(value)
    }
}

impl From<TrustDelta> for i32 {
    fn from(delta: TrustDelta) -> Self {
        delta.0
    }
}

/// Permet d'utiliser `-delta` de manière intuitive
impl Neg for TrustDelta {
    type Output = Self;
    fn neg(self) -> Self::Output {
        self.negate()
    }
}

/// Permet d'afficher le delta proprement dans les logs
impl std::fmt::Display for TrustDelta {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}{}", if self.0 > 0 { "+" } else { "" }, self.0)
    }
}
