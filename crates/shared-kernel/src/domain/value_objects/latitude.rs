// crates/shared_kernel/src/domain/value_objects/latitude.rs

use crate::core::{Error, Result};
use crate::domain::value_objects::ValueObject;
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
            return Err(Error::validation(
                "latitude",
                "Range must be between -90 and 90",
            ));
        }
        Ok(())
    }
}

impl FromStr for Latitude {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self> {
        let val = s
            .trim()
            .parse::<f64>()
            .map_err(|_| Error::validation("latitude", "Invalid number format"))?;
        Self::try_new(val)
    }
}
