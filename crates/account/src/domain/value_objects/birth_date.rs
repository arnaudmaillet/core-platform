// crates/account/src/domain/value_objects/birth_date.rs

use chrono::{Datelike, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use shared_kernel::domain::value_objects::ValueObject;
use shared_kernel::errors::{DomainError, Result};
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "NaiveDate", into = "NaiveDate")]
pub struct BirthDate(NaiveDate);

impl BirthDate {
    pub const MIN_AGE: u32 = 13;
    pub const MAX_AGE: u32 = 125;

    pub fn try_new(date: NaiveDate) -> Result<Self> {
        let birth_date = Self(date);
        birth_date.validate()?;
        Ok(birth_date)
    }

    pub fn from_raw(date: NaiveDate) -> Self {
        Self(date)
    }

    pub fn value(&self) -> NaiveDate {
        self.0
    }

    /// Calcule l'âge (gère les années bissextiles)
    pub fn age_at(&self, reference: NaiveDate) -> u32 {
        let mut age = reference.year() - self.0.year();
        if (reference.month(), reference.day()) < (self.0.month(), self.0.day()) {
            age -= 1;
        }

        age as u32
    }

    pub fn has_reached_age(&self, required_age: u32) -> bool {
        self.current_age() >= required_age
    }

    pub fn current_age(&self) -> u32 {
        self.age_at(Utc::now().date_naive())
    }
}

impl ValueObject for BirthDate {
    fn validate(&self) -> Result<()> {
        let now = Utc::now().date_naive();

        if self.0 > now {
            return Err(DomainError::Validation {
                field: "birth_date",
                reason: "Birth date cannot be in the future".into(),
            });
        }

        let age = self.age_at(now);
        if age < Self::MIN_AGE {
            return Err(DomainError::Validation {
                field: "birth_date",
                reason: format!("User must be at least {} years old", Self::MIN_AGE),
            });
        }

        if age > Self::MAX_AGE {
            return Err(DomainError::Validation {
                field: "birth_date",
                reason: "Invalid birth date (exceeds biological limits)".into(),
            });
        }

        Ok(())
    }
}

// --- CONVERSIONS ---

impl TryFrom<NaiveDate> for BirthDate {
    type Error = DomainError;
    fn try_from(date: NaiveDate) -> Result<Self> {
        Self::try_new(date)
    }
}

impl From<BirthDate> for NaiveDate {
    fn from(birth_date: BirthDate) -> Self {
        birth_date.0
    }
}

impl FromStr for BirthDate {
    type Err = DomainError;
    fn from_str(s: &str) -> Result<Self> {
        let parsed_date =
            NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|_| DomainError::Validation {
                field: "birth_date",
                reason: "Invalid date format. Expected YYYY-MM-DD".into(),
            })?;
        Self::try_new(parsed_date)
    }
}

impl std::fmt::Display for BirthDate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
