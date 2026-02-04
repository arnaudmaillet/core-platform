// crates/shared_kernel/src/domain/value_objects/latitude.rs

use crate::domain::value_objects::ValueObject;
use crate::errors::{DomainError, Result};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Latitude(f64);

impl Latitude {
    pub fn try_new(val: f64) -> Result<Self> {
        let lat = Self(val);
        lat.validate()?;
        Ok(lat)
    }

    pub fn from_raw(val: f64) -> Self {
        Self(val)
    }

    pub fn value(&self) -> f64 {
        self.0
    }
}

impl ValueObject for Latitude {
    fn validate(&self) -> Result<()> {
        if !(-90.0..=90.0).contains(&self.0) {
            return Err(DomainError::Validation {
                field: "latitude",
                reason: "Range must be between -90 and 90".to_string(),
            });
        }
        Ok(())
    }
}

impl FromStr for Latitude {
    type Err = DomainError;
    fn from_str(s: &str) -> Result<Self> {
        let val = s
            .trim()
            .parse::<f64>()
            .map_err(|_| DomainError::Validation {
                field: "latitude",
                reason: "Invalid number format".to_string(),
            })?;
        Self::try_new(val)
    }
}
