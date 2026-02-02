// crates/shared_kernel/src/domain/value_objects/geo_point.rs
use serde::{Deserialize, Serialize};
use crate::domain::value_objects::{Latitude, Longitude, ValueObject};
use crate::errors::{DomainError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GeoPoint {
    latitude: Latitude,
    longitude: Longitude,
}

impl GeoPoint {
    pub fn try_new(lon: f64, lat: f64) -> Result<Self> {
        Ok(Self {
            latitude: Latitude::try_new(lat)?,
            longitude: Longitude::try_new(lon)?,
        })
    }

    pub fn from_raw(lon: f64, lat: f64) -> Self {
        Self {
            latitude: Latitude::from_raw(lat),
            longitude: Longitude::from_raw(lon),
        }
    }

    // --- Getters ---
    pub fn lat(&self) -> f64 { self.latitude.value() }
    pub fn lon(&self) -> f64 { self.longitude.value() }

    pub fn distance_to(&self, other: &GeoPoint) -> f64 {
        let earth_radius_meters = 6_371_000.0;

        let phi1 = self.lat().to_radians();
        let phi2 = other.lat().to_radians();

        let delta_phi = (other.lat() - self.lat()).to_radians();
        let delta_lambda = (other.lon() - self.lon()).to_radians();

        let a = (delta_phi / 2.0).sin().powi(2)
            + phi1.cos() * phi2.cos() * (delta_lambda / 2.0).sin().powi(2);

        // Correction : sqrt(a) et non sin(a)
        let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());

        earth_radius_meters * c
    }
}

impl ValueObject for GeoPoint {
    fn validate(&self) -> Result<()> {
        self.latitude.validate()?;
        self.longitude.validate()?;
        Ok(())
    }
}

impl std::str::FromStr for GeoPoint {
    type Err = DomainError;

    fn from_str(s: &str) -> Result<Self> {
        let parts: Vec<&str> = s.split(',').collect();
        if parts.len() != 2 {
            return Err(DomainError::Validation {
                field: "geopoint",
                reason: "Format 'lon,lat' expected".to_string()
            });
        }

        // On inverse ici pour être cohérent avec le reste du struct
        let lon = Longitude::from_str(parts[0])?;
        let lat = Latitude::from_str(parts[1])?;

        Ok(Self { latitude: lat, longitude: lon })
    }
}