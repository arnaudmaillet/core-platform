/// A strictly positive integer weight assigned to a reaction kind.
///
/// Denormalized into both Redis (`engagement:r:{post}:{profile}` HASH) and
/// ScyllaDB (`post_reactions.weight`) at reaction time. This ensures correct
/// delta reversal even if the weight matrix is reconfigured between a reaction
/// being applied and a later swap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ReactionWeight(i64);

impl ReactionWeight {
    pub fn new(v: i64) -> Self {
        debug_assert!(v > 0, "reaction weight must be positive");
        Self(v)
    }

    pub fn value(self) -> i64 {
        self.0
    }
}
