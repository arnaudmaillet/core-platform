use crate::domain::repositories::ProfileStatsRepository;
use crate::domain::value_objects::{ProfileId, ProfileStats};
use async_trait::async_trait;
use shared_kernel::domain::value_objects::RegionCode;
use shared_kernel::errors::Result;
use std::collections::HashMap;
use std::sync::Mutex;

pub struct ProfileStatsRepositoryStub {
    pub stats: Mutex<HashMap<(ProfileId, RegionCode), ProfileStats>>,
    pub fail_count: Mutex<i32>,
}

impl Default for ProfileStatsRepositoryStub {
    fn default() -> Self {
        Self {
            stats: Mutex::new(HashMap::new()),
            fail_count: Mutex::new(0),
        }
    }
}

#[async_trait]
impl ProfileStatsRepository for ProfileStatsRepositoryStub {
    async fn fetch(
        &self,
        profile_id: &ProfileId,
        region: &RegionCode,
    ) -> Result<Option<ProfileStats>> {
        let map = self.stats.lock().unwrap();
        Ok(map.get(&(profile_id.clone(), region.clone())).cloned())
    }

    async fn save(
        &self,
        profile_id: &ProfileId,
        region: &RegionCode,
        follower_delta: i64,
        following_delta: i64,
        _post_delta: i64,
    ) -> Result<()> {
        {
            let mut fails = self.fail_count.lock().unwrap();
            if *fails > 0 {
                *fails -= 1;
                return Err(shared_kernel::errors::DomainError::ConcurrencyConflict {
                    reason: "Simulated concurrency conflict".into(),
                });
            }
        }

        let mut map = self.stats.lock().unwrap();
        let key = (profile_id.clone(), region.clone());

        // Utilisation de entry pour modifier l'élément en place
        let stats = map.entry(key).or_insert_with(|| ProfileStats::new(0, 0, 0));

        apply_delta_to_stats(stats, follower_delta, following_delta);

        Ok(())
    }

    async fn delete(&self, profile_id: &ProfileId, region: &RegionCode) -> Result<()> {
        let mut map = self.stats.lock().unwrap();
        map.remove(&(profile_id.clone(), region.clone()));
        Ok(())
    }
}

/// Helper pour appliquer des deltas sur l'objet encapsulé
fn apply_delta_to_stats(stats: &mut ProfileStats, f_delta: i64, ing_delta: i64) {
    // Delta followers
    if f_delta > 0 {
        for _ in 0..f_delta {
            stats.increment_followers();
        }
    } else {
        for _ in 0..f_delta.abs() {
            stats.decrement_followers();
        }
    }

    // Delta following
    if ing_delta > 0 {
        for _ in 0..ing_delta {
            stats.increment_following();
        }
    } else {
        for _ in 0..ing_delta.abs() {
            stats.decrement_following();
        }
    }
}
