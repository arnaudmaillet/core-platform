// crates/identity/src/value_objects/birth_date.rs

use crate::{DomainError, Result};
use serde::{Deserialize, Serialize};
use chrono::Utc;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BirthDate(chrono::NaiveDate);

impl BirthDate {
    pub fn new(date: chrono::NaiveDate) -> Result<Self> {
        let today = Utc::now().date_naive();
        let age = today.years_since(date).unwrap_or(0);

        if age < 13 {
            return Err(DomainError::Validation {
                field: "birth_date",
                reason: "user must be at least 13 years old".into(),
            });
        }

        if date > today {
            return Err(DomainError::Validation {
                field: "birth_date",
                reason: "birth date cannot be in the future".into(),
            });
        }

        Ok(Self(date))
    }

    pub fn as_date(&self) -> chrono::NaiveDate {
        self.0
    }
}