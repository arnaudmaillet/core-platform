use crate::core::{Error, Result};
use crate::domain::value_objects::ValueObject;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Speed(f32);

impl Speed {
    pub fn try_new(value: f32) -> Result<Self> {
        let speed = Self(value);
        speed.validate()?;
        Ok(speed)
    }

    pub fn from_raw(value: f32) -> Self {
        Self(value)
    }

    pub fn value(&self) -> f32 {
        self.0
    }
    pub fn to_kmh(&self) -> f32 {
        self.0 * 3.6
    }
}

impl ValueObject for Speed {
    fn validate(&self) -> Result<()> {
        if self.0 < 0.0 {
            return Err(Error::validation("speed", "Speed cannot be negative"));
        }
        Ok(())
    }
}

impl TryFrom<f32> for Speed {
    type Error = Error;

    fn try_from(value: f32) -> Result<Self> {
        let safe_val = if value < 0.0 { 0.0 } else { value };
        Self::try_new(safe_val)
    }
}
