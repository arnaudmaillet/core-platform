// crates/account/src/domain/types/trust_amount.rs

use serde::{Deserialize, Serialize};
use shared_kernel::core::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TrustAmount(u32); // 💡 u32 natif : mathématiquement impossible d'être négatif !

impl TrustAmount {
    pub const PENALTY_BAN: TrustAmount = TrustAmount::from_raw(100); // Un ban détruit le score
    pub const REWARD_UNBAN: TrustAmount = TrustAmount::from_raw(20); // Bonus de réhabilitation
    pub const REWARD_UNSUSPEND: TrustAmount = TrustAmount::from_raw(5); // Petit bonus après suspension

    /// Constructeur interne constant pour les invariants connus (les constantes ci-dessus)
    const fn from_raw(value: u32) -> Self {
        Self(value)
    }

    /// Constructeur sécurisé pour le runtime (interdit le score à 0 si le métier l'impose, ou valide des limites)
    pub fn try_new(value: u32) -> Result<Self, Error> {
        if value == 0 {
            return Err(Error::validation(
                "trust_amount",
                "Trust adjustment amount cannot be zero",
            ));
        }
        Ok(Self(value))
    }

    /// Retourne la valeur brute sous forme de u32 (magnitude pure)
    pub fn value(&self) -> u32 {
        self.0
    }
}

// --- ERGONOMIE & CONVERSIONS ---

impl TryFrom<u32> for TrustAmount {
    type Error = Error;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        Self::try_new(value)
    }
}

impl From<TrustAmount> for u32 {
    fn from(amount: TrustAmount) -> Self {
        amount.0
    }
}

/// Permet d'afficher la quantité proprement dans les logs
impl std::fmt::Display for TrustAmount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} pts", self.0)
    }
}
