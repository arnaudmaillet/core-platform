use crate::core::{Error, Result, ValueObject};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LocationAccuracy(f32);

impl LocationAccuracy {
    pub fn try_new(value: f32) -> Result<Self> {
        let acc = Self(value);
        acc.validate()?;
        Ok(acc)
    }

    pub fn from_raw(value: f32) -> Self {
        Self(value)
    }

    pub fn value(&self) -> f32 {
        self.0
    }
}

impl ValueObject for LocationAccuracy {
    fn validate(&self) -> Result<()> {
        if self.0 < 0.0 {
            return Err(Error::validation("accuracy", "Accuracy cannot be negative"));
        }
        Ok(())
    }
}

impl TryFrom<f32> for LocationAccuracy {
    type Error = Error;

    fn try_from(value: f32) -> Result<Self> {
        let safe_val = if value < 0.0 { 0.0 } else { value };
        Self::try_new(safe_val)
    }
}
