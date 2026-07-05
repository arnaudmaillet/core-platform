use crate::error::GeoDiscoveryError;

/// A validated WGS-84 geographic coordinate pair.
///
/// Invariants (enforced at construction):
///   lat ∈ [-90.0, 90.0]
///   lng ∈ [-180.0, 180.0]
///   Neither value is NaN or infinite.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GeoCoordinate {
    pub lat: f64,
    pub lng: f64,
}

impl GeoCoordinate {
    pub fn new(lat: f64, lng: f64) -> Result<Self, GeoDiscoveryError> {
        if !lat.is_finite() || !(-90.0..=90.0).contains(&lat)
            || !lng.is_finite() || !(-180.0..=180.0).contains(&lng)
        {
            return Err(GeoDiscoveryError::InvalidCoordinate { lat, lng });
        }
        Ok(Self { lat, lng })
    }
}
