//! Author tier classification, derived from follower count.
//!
//! Social-graph is the **producer** of the fleet's author-tier signal: it owns
//! follower counts (it sees every follow / unfollow), so it is the service that
//! classifies an author into Standard / Premium / VIP and emits a tier-change
//! event when a follow crosses a band. Downstream, `profile` persists the tier and
//! `post` denormalizes it onto its events; `timeline` and `geo-discovery` route on
//! it. The numeric taxonomy (0/1/2) mirrors those consumers.

/// An author's tier. The numeric value is the wire contract shared with
/// `timeline` / `geo-discovery` (`0=Standard`, `1=Premium`, `2=Vip`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum AuthorTier {
    Standard = 0,
    Premium = 1,
    Vip = 2,
}

impl AuthorTier {
    pub fn as_u8(self) -> u8 {
        self as u8
    }

    /// Classify a follower count against the thresholds.
    pub fn from_follower_count(count: i64, thresholds: TierThresholds) -> Self {
        if count >= thresholds.vip {
            AuthorTier::Vip
        } else if count >= thresholds.premium {
            AuthorTier::Premium
        } else {
            AuthorTier::Standard
        }
    }

    /// The new tier **iff** a follower-count change from `old_count` to `new_count`
    /// crossed a tier boundary; `None` when the tier is unchanged.
    ///
    /// Stateless — it relies only on the two counts. Because Redis `INCR`/`DECR`
    /// return distinct sequential values, exactly one follow/unfollow observes any
    /// given boundary crossing, so this neither misses nor double-emits.
    pub fn crossing(
        old_count: i64,
        new_count: i64,
        thresholds: TierThresholds,
    ) -> Option<AuthorTier> {
        let old = Self::from_follower_count(old_count, thresholds);
        let new = Self::from_follower_count(new_count, thresholds);
        (old != new).then_some(new)
    }
}

/// Follower-count thresholds for tier classification — the Premium and VIP floors.
/// These are config-driven (a product decision); the defaults live at the
/// composition root.
#[derive(Debug, Clone, Copy)]
pub struct TierThresholds {
    pub premium: i64,
    pub vip: i64,
}

impl TierThresholds {
    /// Build thresholds, clamping to the invariant `1 <= premium <= vip` so a
    /// misconfiguration can never invert or zero the bands.
    pub fn new(premium: i64, vip: i64) -> Self {
        let premium = premium.max(1);
        let vip = vip.max(premium);
        Self { premium, vip }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn thresholds() -> TierThresholds {
        TierThresholds::new(10_000, 1_000_000)
    }

    #[test]
    fn classifies_by_band() {
        let t = thresholds();
        assert_eq!(AuthorTier::from_follower_count(0, t), AuthorTier::Standard);
        assert_eq!(AuthorTier::from_follower_count(9_999, t), AuthorTier::Standard);
        assert_eq!(AuthorTier::from_follower_count(10_000, t), AuthorTier::Premium);
        assert_eq!(AuthorTier::from_follower_count(999_999, t), AuthorTier::Premium);
        assert_eq!(AuthorTier::from_follower_count(1_000_000, t), AuthorTier::Vip);
    }

    #[test]
    fn detects_an_upward_crossing_only_at_the_boundary() {
        let t = thresholds();
        // 9_999 → 10_000 crosses into Premium.
        assert_eq!(AuthorTier::crossing(9_999, 10_000, t), Some(AuthorTier::Premium));
        // 10_000 → 10_001 stays Premium — no emit.
        assert_eq!(AuthorTier::crossing(10_000, 10_001, t), None);
        // 999_999 → 1_000_000 crosses into Vip.
        assert_eq!(AuthorTier::crossing(999_999, 1_000_000, t), Some(AuthorTier::Vip));
    }

    #[test]
    fn detects_a_downward_crossing() {
        let t = thresholds();
        // 1_000_000 → 999_999 drops back to Premium.
        assert_eq!(AuthorTier::crossing(1_000_000, 999_999, t), Some(AuthorTier::Premium));
        // 10_000 → 9_999 drops to Standard.
        assert_eq!(AuthorTier::crossing(10_000, 9_999, t), Some(AuthorTier::Standard));
    }

    #[test]
    fn no_crossing_within_a_band() {
        let t = thresholds();
        assert_eq!(AuthorTier::crossing(5, 6, t), None);
        assert_eq!(AuthorTier::crossing(500_000, 500_001, t), None);
    }

    #[test]
    fn thresholds_cannot_invert() {
        // Misconfigured (vip < premium) is clamped so vip >= premium >= 1.
        let t = TierThresholds::new(0, -5);
        assert_eq!(t.premium, 1);
        assert!(t.vip >= t.premium);
    }
}
