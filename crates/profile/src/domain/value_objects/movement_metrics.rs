// crates/profile/src/domain/value_objects/movement.rs

use serde::{Deserialize, Serialize};
use shared_kernel::domain::value_objects::{Heading, Speed, ValueObject};
use shared_kernel::errors::Result;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MovementMetrics {
    speed: Speed,
    heading: Heading,
}

impl MovementMetrics {
    pub fn try_new(speed: Speed, heading: Heading) -> Result<Self> {
        let metrics = Self { speed, heading };
        metrics.validate()?;
        Ok(metrics)
    }

    /// Pour l'infrastructure (Reconstruction DB)
    pub fn from_raw(speed: Speed, heading: Heading) -> Self {
        Self { speed, heading }
    }

    /// Pour les Use Cases (Primitifs -> VO)
    pub fn try_from_primitives(speed: f32, heading: f32) -> Result<Self> {
        Self::try_new(Speed::try_new(speed)?, Heading::try_new(heading)?)
    }

    // --- Getters (Accès en lecture seule) ---

    pub fn speed(&self) -> Speed {
        self.speed
    }

    pub fn heading(&self) -> Heading {
        self.heading
    }
}

impl ValueObject for MovementMetrics {
    fn validate(&self) -> Result<()> {
        self.speed.validate()?;
        self.heading.validate()?;
        Ok(())
    }
}
