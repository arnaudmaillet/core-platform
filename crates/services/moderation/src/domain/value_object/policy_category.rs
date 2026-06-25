use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::ModerationError;

/// Normalized integrity taxonomy. Policy-version-agnostic at the type level: the
/// *thresholds and consequences* for each category live in the pinned policy
/// version (see [`PenaltyPolicy`](crate::domain::value_object::PenaltyPolicy)),
/// never hard-coded here. Only the two structural predicates below — which
/// categories are zero-tolerance and which are appealable — are intrinsic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyCategory {
    Spam,
    Harassment,
    Hate,
    /// Terrorist & violent extremist content.
    ViolentExtremism,
    /// Child sexual abuse material.
    Csam,
    /// Non-consensual intimate imagery.
    Ncii,
    SelfHarm,
    Misinformation,
    Other,
}

impl PolicyCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Spam => "spam",
            Self::Harassment => "harassment",
            Self::Hate => "hate",
            Self::ViolentExtremism => "violent_extremism",
            Self::Csam => "csam",
            Self::Ncii => "ncii",
            Self::SelfHarm => "self_harm",
            Self::Misinformation => "misinformation",
            Self::Other => "other",
        }
    }

    /// Catastrophic-harm categories that are eligible for the synchronous,
    /// fail-closed Screen gate (Plane C) and that must never be published
    /// optimistically. The hot path treats an unavailable screen for these as a
    /// hard block.
    pub fn is_zero_tolerance(&self) -> bool {
        matches!(self, Self::Csam | Self::Ncii | Self::ViolentExtremism)
    }

    /// Whether a decision in this category is user-appealable. Legally-mandated
    /// removals (CSAM) are not appealable through the normal flow.
    pub fn is_appealable(&self) -> bool {
        !matches!(self, Self::Csam)
    }
}

impl fmt::Display for PolicyCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl TryFrom<&str> for PolicyCategory {
    type Error = ModerationError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "spam" => Ok(Self::Spam),
            "harassment" => Ok(Self::Harassment),
            "hate" => Ok(Self::Hate),
            "violent_extremism" => Ok(Self::ViolentExtremism),
            "csam" => Ok(Self::Csam),
            "ncii" => Ok(Self::Ncii),
            "self_harm" => Ok(Self::SelfHarm),
            "misinformation" => Ok(Self::Misinformation),
            "other" => Ok(Self::Other),
            other => Err(ModerationError::UnknownPolicyCategory {
                category: other.to_owned(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_tolerance_set_is_exactly_the_catastrophic_three() {
        for c in [
            PolicyCategory::Csam,
            PolicyCategory::Ncii,
            PolicyCategory::ViolentExtremism,
        ] {
            assert!(c.is_zero_tolerance(), "{c} must be zero-tolerance");
        }
        for c in [
            PolicyCategory::Spam,
            PolicyCategory::Harassment,
            PolicyCategory::Hate,
            PolicyCategory::SelfHarm,
            PolicyCategory::Misinformation,
            PolicyCategory::Other,
        ] {
            assert!(!c.is_zero_tolerance(), "{c} must not be zero-tolerance");
        }
    }

    #[test]
    fn csam_is_not_appealable() {
        assert!(!PolicyCategory::Csam.is_appealable());
        assert!(PolicyCategory::Harassment.is_appealable());
        assert!(PolicyCategory::Ncii.is_appealable());
    }

    #[test]
    fn string_round_trip() {
        for c in [
            PolicyCategory::Spam,
            PolicyCategory::Harassment,
            PolicyCategory::Hate,
            PolicyCategory::ViolentExtremism,
            PolicyCategory::Csam,
            PolicyCategory::Ncii,
            PolicyCategory::SelfHarm,
            PolicyCategory::Misinformation,
            PolicyCategory::Other,
        ] {
            assert_eq!(PolicyCategory::try_from(c.as_str()).unwrap(), c);
        }
        assert!(matches!(
            PolicyCategory::try_from("nope").unwrap_err(),
            ModerationError::UnknownPolicyCategory { .. }
        ));
    }
}
