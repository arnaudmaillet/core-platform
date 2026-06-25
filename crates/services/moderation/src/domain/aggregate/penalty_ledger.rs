use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::value_object::{
    ActionType, ActorId, PenaltyPolicy, PolicyCategory, Strike,
};

/// The **PenaltyLedger** aggregate — the graduated-enforcement engine for an
/// actor. It accumulates [`Strike`]s (each with a snapshotted point value and a
/// decay deadline) and, given a [`PenaltyPolicy`], deterministically recommends
/// the actor-level action the accumulated history warrants.
///
/// This is the brain of the service, and it is pure: the same ledger + the same
/// policy + the same `now` always yields the same recommendation, which is what
/// makes graduated enforcement auditable and unit-testable. The engine recommends;
/// the application layer decides whether to act on the recommendation.
///
/// Decay is intrinsic: only strikes whose `expires_at` is still in the future at
/// `now` count toward the active total, so an actor's standing recovers over time
/// without any sweep job mutating the ledger.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PenaltyLedger {
    actor_id: ActorId,
    strikes: Vec<Strike>,
    version: i64,
}

impl PenaltyLedger {
    /// An empty ledger for an actor with no history.
    pub fn empty(actor_id: ActorId) -> Self {
        Self { actor_id, strikes: Vec::new(), version: 0 }
    }

    /// Reconstructs from storage.
    pub fn reconstitute(actor_id: ActorId, strikes: Vec<Strike>, version: i64) -> Self {
        Self { actor_id, strikes, version }
    }

    pub fn actor_id(&self) -> ActorId {
        self.actor_id
    }

    pub fn version(&self) -> i64 {
        self.version
    }

    pub fn strikes(&self) -> &[Strike] {
        &self.strikes
    }

    /// Records a strike in `category`, weighting it per the policy in force *now*
    /// and stamping its decay deadline. Points and the deadline are snapshotted so
    /// a later policy change does not retroactively re-weight it.
    pub fn record_strike(
        &mut self,
        category: PolicyCategory,
        now: DateTime<Utc>,
        policy: &PenaltyPolicy,
    ) {
        let points = policy.weight_for(category);
        let expires_at = now + policy.decay_window();
        self.strikes.push(Strike::new(category, points, now, expires_at));
        self.version += 1;
    }

    /// The sum of points from strikes that have not yet decayed at `now`.
    pub fn active_points(&self, now: DateTime<Utc>) -> u32 {
        self.strikes
            .iter()
            .filter(|s| s.is_active(now))
            .map(|s| s.points())
            .sum()
    }

    /// The number of strikes still counting at `now`.
    pub fn active_strike_count(&self, now: DateTime<Utc>) -> usize {
        self.strikes.iter().filter(|s| s.is_active(now)).count()
    }

    /// The actor-level action the accumulated, non-decayed history warrants under
    /// `policy`. `NoAction` when below the lowest escalation tier.
    pub fn recommended_action(&self, now: DateTime<Utc>, policy: &PenaltyPolicy) -> ActionType {
        policy.recommended_action(self.active_points(now))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use uuid::Uuid;

    fn t0() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-06-25T12:00:00Z").unwrap().with_timezone(&Utc)
    }

    fn ledger() -> PenaltyLedger {
        PenaltyLedger::empty(ActorId::from_uuid(Uuid::from_u128(42)))
    }

    #[test]
    fn empty_ledger_recommends_no_action() {
        let l = ledger();
        let p = PenaltyPolicy::standard();
        assert_eq!(l.active_points(t0()), 0);
        assert_eq!(l.recommended_action(t0(), &p), ActionType::NoAction);
    }

    #[test]
    fn strikes_accumulate_and_escalate() {
        let mut l = ledger();
        let p = PenaltyPolicy::standard(); // spam=1, harassment=2, hate=3; ladder 1→Warn 3→Restrict 5→Suspend 6→Ban
        l.record_strike(PolicyCategory::Spam, t0(), &p); // 1 pt
        assert_eq!(l.recommended_action(t0(), &p), ActionType::Warn);
        l.record_strike(PolicyCategory::Harassment, t0(), &p); // +2 = 3 pts
        assert_eq!(l.active_points(t0()), 3);
        assert_eq!(l.recommended_action(t0(), &p), ActionType::RestrictActor);
        l.record_strike(PolicyCategory::Harassment, t0(), &p); // +2 = 5 pts
        assert_eq!(l.recommended_action(t0(), &p), ActionType::Suspend);
    }

    #[test]
    fn a_single_catastrophic_strike_reaches_ban() {
        let mut l = ledger();
        let p = PenaltyPolicy::standard(); // csam weight 6, ban threshold 6
        l.record_strike(PolicyCategory::Csam, t0(), &p);
        assert_eq!(l.active_points(t0()), 6);
        assert_eq!(l.recommended_action(t0(), &p), ActionType::Ban);
    }

    #[test]
    fn decayed_strikes_stop_counting() {
        let mut l = ledger();
        let p = PenaltyPolicy::standard(); // 90-day decay
        l.record_strike(PolicyCategory::Hate, t0(), &p); // 3 pts
        assert_eq!(l.recommended_action(t0(), &p), ActionType::RestrictActor);

        // Just before decay: still counts.
        let before = t0() + Duration::days(90) - Duration::seconds(1);
        assert_eq!(l.active_points(before), 3);

        // At/after the decay deadline: drops to zero ⇒ standing recovers.
        let after = t0() + Duration::days(90);
        assert_eq!(l.active_points(after), 0);
        assert_eq!(l.active_strike_count(after), 0);
        assert_eq!(l.recommended_action(after, &p), ActionType::NoAction);
    }

    #[test]
    fn snapshot_weight_survives_a_policy_change() {
        let mut l = ledger();
        let lenient = PenaltyPolicy::new(
            Duration::days(90),
            1,
            vec![(PolicyCategory::Spam, 1)],
            vec![(1, ActionType::Warn), (5, ActionType::Suspend)],
        )
        .unwrap();
        l.record_strike(PolicyCategory::Spam, t0(), &lenient); // snapshotted at 1 pt

        // A later, harsher policy weights spam at 5 — but the existing strike keeps
        // its snapshotted value; only future strikes get the new weight.
        let harsh = PenaltyPolicy::new(
            Duration::days(90),
            1,
            vec![(PolicyCategory::Spam, 5)],
            vec![(1, ActionType::Warn), (5, ActionType::Suspend)],
        )
        .unwrap();
        assert_eq!(l.active_points(t0()), 1, "existing strike keeps snapshot weight");
        l.record_strike(PolicyCategory::Spam, t0(), &harsh); // +5 = 6
        assert_eq!(l.active_points(t0()), 6);
        assert_eq!(l.recommended_action(t0(), &harsh), ActionType::Suspend);
    }

    #[test]
    fn recording_bumps_version() {
        let mut l = ledger();
        let p = PenaltyPolicy::standard();
        assert_eq!(l.version(), 0);
        l.record_strike(PolicyCategory::Spam, t0(), &p);
        l.record_strike(PolicyCategory::Spam, t0(), &p);
        assert_eq!(l.version(), 2);
    }
}
