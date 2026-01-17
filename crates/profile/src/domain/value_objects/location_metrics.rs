use serde::{Deserialize, Serialize};
use shared_kernel::domain::value_objects::{Altitude, LocationAccuracy, ValueObject};
use shared_kernel::errors::Result;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LocationMetrics {
    accuracy: LocationAccuracy,
    altitude: Option<Altitude>,
}

impl LocationMetrics {
    /// Constructeur principal (Domaine)
    pub fn try_new(accuracy: LocationAccuracy, altitude: Option<Altitude>) -> Result<Self> {
        let metrics = Self { accuracy, altitude };
        metrics.validate()?;
        Ok(metrics)
    }

    /// Pour l'infrastructure (Reconstruction DB sans validation)
    pub fn new_unchecked(accuracy: LocationAccuracy, altitude: Option<Altitude>) -> Self {
        Self { accuracy, altitude }
    }

    /// Pour les Use Cases / Commandes (Conversion facilitée depuis f32)
    pub fn try_from_primitives(accuracy: f32, altitude: Option<f32>) -> Result<Self> {
        Ok(Self {
            accuracy: LocationAccuracy::try_new(accuracy)?,
            // .transpose() transforme Option<Result<T>> en Result<Option<T>>
            altitude: altitude.map(Altitude::try_new).transpose()?,
        })
    }

    // --- Getters ---

    pub fn accuracy(&self) -> LocationAccuracy {
        self.accuracy
    }

    pub fn altitude(&self) -> Option<Altitude> {
        self.altitude
    }
}

impl ValueObject for LocationMetrics {
    fn validate(&self) -> Result<()> {
        self.accuracy.validate()?;
        if let Some(alt) = self.altitude {
            alt.validate()?;
        }
        Ok(())
    }
}