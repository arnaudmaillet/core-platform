use serde::{Deserialize, Serialize};
use shared_kernel::domain::value_objects::Counter;

/// Statistiques dénormalisées du profil
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProfileStats {
    pub follower_count: Counter,
    pub following_count: Counter,
}