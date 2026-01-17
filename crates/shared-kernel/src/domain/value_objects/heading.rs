use serde::{Deserialize, Serialize};
use crate::domain::value_objects::ValueObject;
use crate::errors::{DomainError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Heading(f32);

impl Heading {
    pub fn try_new(value: f32) -> Result<Self> {
        let heading = Self(value);
        heading.validate()?; // On vérifie si c'est STRICTEMENT entre 0 et 360
        Ok(heading)
    }

    pub fn new_unchecked(value: f32) -> Self {
        Self(value)
    }

    pub fn value(&self) -> f32 { self.0 }
}

impl ValueObject for Heading {
    fn validate(&self) -> Result<()> {
        if !(0.0..=360.0).contains(&self.0) {
            return Err(DomainError::Validation {
                field: "heading",
                reason: format!("Value {} must be between 0 and 360", self.0),
            });
        }
        Ok(())
    }
}

impl TryFrom<f32> for Heading {
    type Error = DomainError;

    fn try_from(value: f32) -> Result<Self> {
        // L'INFRASTRUCTURE accepte de redresser la donnée
        let normalized = value.rem_euclid(360.0);
        Self::try_new(normalized) // Puis on passe par le constructeur strict
    }
}