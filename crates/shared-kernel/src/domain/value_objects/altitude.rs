use crate::domain::value_objects::ValueObject;
use crate::errors::{DomainError, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Altitude(f32);

impl Altitude {
    pub fn try_new(value: f32) -> Result<Self> {
        Ok(Self(value))
    }

    pub fn from_raw(value: f32) -> Self {
        Self(value)
    }

    pub fn value(&self) -> f32 {
        self.0
    }
}

impl ValueObject for Altitude {
    fn validate(&self) -> Result<()> {
        Ok(())
    }
}

impl TryFrom<f32> for Altitude {
    type Error = DomainError;

    fn try_from(value: f32) -> Result<Self> {
        Self::try_new(value)
    }
}
