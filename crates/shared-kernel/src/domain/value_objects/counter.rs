// crates/shared_kernel/src/domain/value_objects/counter.rs

use crate::domain::value_objects::ValueObject;
use crate::errors::{DomainError, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Serialize, Deserialize)]
pub struct Counter(u64);

impl Counter {
    pub fn try_new(val: u64) -> Result<Self> {
        let counter = Self(val);
        counter.validate()?;
        Ok(counter)
    }

    /// Pour la reconstruction depuis la DB
    pub fn from_raw(val: u64) -> Self {
        Self(val)
    }

    /// Incrément sécurisé contre l'overflow (Saturating)
    /// On préfère saturer au Max plutôt que de faire crasher le système
    pub fn increment(&mut self) {
        self.0 = self.0.saturating_add(1);
    }

    /// Décrément sécurisé (ne descendra jamais sous 0)
    pub fn decrement(&mut self) {
        self.0 = self.0.saturating_sub(1);
    }

    pub fn value(&self) -> u64 {
        self.0
    }

    pub fn is_zero(&self) -> bool {
        self.0 == 0
    }
}

impl ValueObject for Counter {
    fn validate(&self) -> Result<()> {
        Ok(())
    }
}

impl TryFrom<u64> for Counter {
    type Error = DomainError;

    fn try_from(val: u64) -> Result<Self> {
        Self::try_new(val)
    }
}

/// Permet de faire : domain.post_count().into() vers un u64 (infaillible)
impl From<Counter> for u64 {
    fn from(counter: Counter) -> Self {
        counter.0
    }
}