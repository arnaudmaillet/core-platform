// crates/geo_discovery/src/domain/types/popularity_score.rs

use serde::{Deserialize, Serialize};
use shared_kernel::core::{Error, Result};
use std::fmt;

/// Value Object représentant le score algorithmique de popularité d'un post.
/// Gère l'encapsulation d'un f64 sous forme de type fort (Newtype pattern)
/// pour éviter l'obsession des primitifs sans aucun coût à l'exécution.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct PopularityScore(f64);

impl PopularityScore {
    pub fn try_new(value: f64) -> Result<Self> {
        if value.is_nan() || value.is_infinite() {
            return Err(Error::validation(
                "popularity_score",
                "Le score de popularité ne peut pas être NaN ou infini.",
            ));
        }
        let sanitized_value = if value < 0.0 { 0.0 } else { value };

        Ok(Self(sanitized_value))
    }

    pub fn from_raw(value: f64) -> Self {
        Self(value)
    }

    pub fn value(&self) -> f64 {
        self.0
    }

    pub fn apply_delta(&mut self, delta: f64) -> Result<()> {
        let new_value = self.0 + delta;
        if new_value.is_nan() || new_value.is_infinite() {
            return Err(Error::validation(
                "popularity_score",
                "L'application du delta produit une valeur invalide (NaN/Inf).",
            ));
        }

        self.0 = if new_value < 0.0 { 0.0 } else { new_value };
        Ok(())
    }
}

impl Default for PopularityScore {
    fn default() -> Self {
        Self(1.0)
    }
}

impl fmt::Display for PopularityScore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.4}", self.0)
    }
}