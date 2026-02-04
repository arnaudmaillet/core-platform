// crates/shared_kernel/src/domain/value_objects/longitude.rs

use crate::domain::value_objects::ValueObject;
use crate::errors::{DomainError, Result};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Longitude(f64);

impl Longitude {
    pub fn try_new(val: f64) -> Result<Self> {
        let lon = Self(val);
        lon.validate()?;
        Ok(lon)
    }

    pub fn from_raw(val: f64) -> Self {
        Self(val)
    }

    pub fn value(&self) -> f64 {
        self.0
    }
}

impl ValueObject for Longitude {
    fn validate(&self) -> Result<()> {
        if !(-180.0..=180.0).contains(&self.0) {
            return Err(DomainError::Validation {
                field: "longitude",
                reason: "Range must be between -180 and 180".to_string(),
            });
        }
        Ok(())
    }
}

impl FromStr for Longitude {
    type Err = DomainError;
    fn from_str(s: &str) -> Result<Self> {
        let val = s
            .trim()
            .parse::<f64>()
            .map_err(|_| DomainError::Validation {
                field: "longitude",
                reason: "Invalid number format".to_string(),
            })?;
        Self::try_new(val)
    }
}
