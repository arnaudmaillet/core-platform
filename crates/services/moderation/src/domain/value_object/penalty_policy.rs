use chrono::Duration;
use serde::{Deserialize, Serialize};

use crate::domain::value_object::{ActionType, PolicyCategory};
use crate::error::ModerationError;

/// The tunable inputs to the graduated-enforcement engine, supplied by the pinned
/// policy version. Keeping these *out* of the domain types (and in a value object
/// the application injects) is what makes the
/// [`PenaltyLedger`](crate::domain::aggregate::PenaltyLedger) deterministic and
/// auditable: the same ledger + the same policy always yields the same
/// recommendation.
///
/// Three knobs:
/// * `decay_window` — how long a strike counts toward the active total.
/// * per-category point `weights` (with a `default_weight` fallback) — how heavily
///   each category is penalised.
/// * `escalation` tiers — `(min_active_points, action)` pairs; the recommended
///   action is the harshest tier whose threshold the actor has reached.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PenaltyPolicy {
    decay_window: Duration,
    default_weight: u32,
    weights: Vec<(PolicyCategory, u32)>,
    /// Sorted ascending by threshold on construction.
    escalation: Vec<(u32, ActionType)>,
}

impl PenaltyPolicy {
    /// Constructs a policy. Rejects an empty escalation ladder or a zero decay
    /// window; normalises the ladder into ascending threshold order.
    pub fn new(
        decay_window: Duration,
        default_weight: u32,
        weights: Vec<(PolicyCategory, u32)>,
        mut escalation: Vec<(u32, ActionType)>,
    ) -> Result<Self, ModerationError> {
        if decay_window <= Duration::zero() {
            return Err(ModerationError::DomainViolation {
                field: "penalty_policy.decay_window".into(),
                message: "decay window must be positive".into(),
            });
        }
        if escalation.is_empty() {
            return Err(ModerationError::DomainViolation {
                field: "penalty_policy.escalation".into(),
                message: "escalation ladder must have at least one tier".into(),
            });
        }
        escalation.sort_by_key(|(threshold, _)| *threshold);
        Ok(Self { decay_window, default_weight, weights, escalation })
    }

    /// A sensible default ladder for bootstrapping and tests. Real deployments
    /// supply their own, pinned to a policy version.
    pub fn standard() -> Self {
        // Safe to unwrap: the literal arguments satisfy every invariant.
        Self::new(
            Duration::days(90),
            1,
            vec![
                (PolicyCategory::Spam, 1),
                (PolicyCategory::Harassment, 2),
                (PolicyCategory::Hate, 3),
                (PolicyCategory::ViolentExtremism, 6),
                (PolicyCategory::Csam, 6),
                (PolicyCategory::Ncii, 6),
            ],
            vec![
                (1, ActionType::Warn),
                (3, ActionType::RestrictActor),
                (5, ActionType::Suspend),
                (6, ActionType::Ban),
            ],
        )
        .expect("standard penalty policy is valid")
    }

    pub fn decay_window(&self) -> Duration {
        self.decay_window
    }

    /// Point weight for a category, falling back to `default_weight`.
    pub fn weight_for(&self, category: PolicyCategory) -> u32 {
        self.weights
            .iter()
            .find(|(c, _)| *c == category)
            .map(|(_, w)| *w)
            .unwrap_or(self.default_weight)
    }

    /// The recommended action for a given active point total: the harshest tier
    /// whose threshold is met. Below the lowest threshold ⇒ `NoAction`.
    pub fn recommended_action(&self, active_points: u32) -> ActionType {
        self.escalation
            .iter()
            .filter(|(threshold, _)| active_points >= *threshold)
            .map(|(_, action)| *action)
            .next_back()
            .unwrap_or(ActionType::NoAction)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_degenerate_inputs() {
        assert!(PenaltyPolicy::new(Duration::zero(), 1, vec![], vec![(1, ActionType::Warn)]).is_err());
        assert!(PenaltyPolicy::new(Duration::days(1), 1, vec![], vec![]).is_err());
    }

    #[test]
    fn weight_falls_back_to_default() {
        let p = PenaltyPolicy::new(
            Duration::days(30),
            2,
            vec![(PolicyCategory::Csam, 9)],
            vec![(1, ActionType::Warn)],
        )
        .unwrap();
        assert_eq!(p.weight_for(PolicyCategory::Csam), 9);
        assert_eq!(p.weight_for(PolicyCategory::Spam), 2); // default
    }

    #[test]
    fn recommended_action_climbs_the_ladder() {
        let p = PenaltyPolicy::standard();
        assert_eq!(p.recommended_action(0), ActionType::NoAction);
        assert_eq!(p.recommended_action(1), ActionType::Warn);
        assert_eq!(p.recommended_action(2), ActionType::Warn);
        assert_eq!(p.recommended_action(3), ActionType::RestrictActor);
        assert_eq!(p.recommended_action(5), ActionType::Suspend);
        assert_eq!(p.recommended_action(6), ActionType::Ban);
        assert_eq!(p.recommended_action(100), ActionType::Ban);
    }

    #[test]
    fn escalation_is_normalised_to_ascending_order() {
        // Provide tiers out of order; recommendation must still be monotone.
        let p = PenaltyPolicy::new(
            Duration::days(30),
            1,
            vec![],
            vec![
                (5, ActionType::Suspend),
                (1, ActionType::Warn),
                (3, ActionType::RestrictActor),
            ],
        )
        .unwrap();
        assert_eq!(p.recommended_action(1), ActionType::Warn);
        assert_eq!(p.recommended_action(4), ActionType::RestrictActor);
        assert_eq!(p.recommended_action(5), ActionType::Suspend);
    }
}
