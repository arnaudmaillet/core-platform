use crate::error::GeoDiscoveryError;

/// A non-negative virality score.
///
/// Stored as f64 internally; converted to f32 for ScyllaDB `float` columns
/// and used as f64 for Redis ZADD scores (Redis stores sorted set scores as
/// IEEE 754 doubles).
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct ViralityScore(f64);

impl ViralityScore {
    pub const ZERO: Self = Self(0.0);

    pub fn new(v: f64) -> Result<Self, GeoDiscoveryError> {
        if !v.is_finite() || v < 0.0 {
            return Err(GeoDiscoveryError::DomainViolation {
                field:   "virality_score".to_owned(),
                message: format!("score must be a finite non-negative number, got {v}"),
            });
        }
        Ok(Self(v))
    }

    pub fn as_f64(&self) -> f64 {
        self.0
    }

    pub fn as_f32(&self) -> f32 {
        self.0 as f32
    }

    pub fn exceeds_threshold(&self, threshold: f64) -> bool {
        self.0 >= threshold
    }
}

impl From<f32> for ViralityScore {
    fn from(v: f32) -> Self {
        Self(v as f64)
    }
}
