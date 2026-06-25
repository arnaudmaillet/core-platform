use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::ModerationError;

/// A classifier/aggregate confidence in `[0.0, 1.0]`.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Confidence(f64);

impl Confidence {
    /// Constructs a confidence, rejecting NaN and out-of-range values.
    pub fn new(value: f64) -> Result<Self, ModerationError> {
        if value.is_nan() || !(0.0..=1.0).contains(&value) {
            return Err(ModerationError::DomainViolation {
                field: "confidence".into(),
                message: format!("confidence must be within [0.0, 1.0], got {value}"),
            });
        }
        Ok(Self(value))
    }

    /// Clamps into range instead of failing (for trusted, lossy upstreams).
    pub fn clamped(value: f64) -> Self {
        let v = if value.is_nan() { 0.0 } else { value.clamp(0.0, 1.0) };
        Self(v)
    }

    pub fn value(&self) -> f64 {
        self.0
    }

    /// Whether this meets or exceeds a decision threshold.
    pub fn at_least(&self, threshold: f64) -> bool {
        self.0 >= threshold
    }
}

impl fmt::Display for Confidence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.4}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_in_range_rejects_out_of_range() {
        assert!(Confidence::new(0.0).is_ok());
        assert!(Confidence::new(1.0).is_ok());
        assert!(Confidence::new(0.73).is_ok());
        assert!(Confidence::new(-0.1).is_err());
        assert!(Confidence::new(1.1).is_err());
        assert!(Confidence::new(f64::NAN).is_err());
    }

    #[test]
    fn clamped_never_escapes_range() {
        assert_eq!(Confidence::clamped(2.0).value(), 1.0);
        assert_eq!(Confidence::clamped(-1.0).value(), 0.0);
        assert_eq!(Confidence::clamped(f64::NAN).value(), 0.0);
    }

    #[test]
    fn threshold_check() {
        assert!(Confidence::new(0.9).unwrap().at_least(0.8));
        assert!(!Confidence::new(0.5).unwrap().at_least(0.8));
    }
}
