// crates/shared_kernel/src/domain/value_objects/longitude.rs

use crate::core::{Error, Result};
use crate::domain::value_objects::ValueObject;
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
            return Err(Error::validation(
                "longitude",
                "Range must be between -180 and 180",
            ));
        }
        Ok(())
    }
}

impl FromStr for Longitude {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self> {
        let val = s
            .trim()
            .parse::<f64>()
            .map_err(|_| Error::validation("longitude", "Invalid number format".to_string()))?;
        Self::try_new(val)
    }
}
