use serde::{Deserialize, Serialize};
use shared_kernel::domain::value_objects::Counter;

/// Statistiques dénormalisées du profil.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ProfileStats {
    follower_count: Counter,
    following_count: Counter,
}

impl ProfileStats {
    /// Crée de nouvelles statistiques (généralement à 0)
    pub fn new(follower_count: u64, following_count: u64) -> Self {
        Self {
            follower_count: Counter::from_raw(follower_count),
            following_count: Counter::from_raw(following_count),
        }
    }

    // --- Getters ---

    pub fn follower_count(&self) -> u64 {
        self.follower_count.value()
    }

    pub fn following_count(&self) -> u64 {
        self.following_count.value()
    }

    // --- Mutateurs (Accessibles uniquement par le crate domain) ---
    // On utilise pub(crate) pour que seul l'agrégat Profile puisse les modifier

    pub(crate) fn increment_followers(&mut self) {
        self.follower_count.increment();
    }

    pub(crate) fn decrement_followers(&mut self) {
        self.follower_count.decrement();
    }

    pub(crate) fn increment_following(&mut self) {
        self.following_count.increment();
    }

    pub(crate) fn decrement_following(&mut self) {
        self.following_count.decrement();
    }
}
