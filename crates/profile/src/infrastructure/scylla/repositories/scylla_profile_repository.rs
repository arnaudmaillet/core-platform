// crates/profile/src/infrastructure/scylla/repositories/scylla_profile_repository.rs

use async_trait::async_trait;
use scylla::client::session::Session;
use scylla::value::Counter;
use shared_kernel::domain::Identifier;
use shared_kernel::domain::value_objects::RegionCode;
use shared_kernel::errors::{DomainError, Result};
use std::sync::Arc;
use crate::domain::repositories::ProfileStatsRepository;
use crate::domain::value_objects::{ProfileId, ProfileStats};

pub struct ScyllaProfileRepository {
    session: Arc<Session>,
}

impl ScyllaProfileRepository {
    pub fn new(session: Arc<Session>) -> Self {
        Self { session }
    }
}

#[async_trait]
impl ProfileStatsRepository for ScyllaProfileRepository {
    async fn fetch(
        &self,
        profile_id: &ProfileId,
        region: &RegionCode,
    ) -> Result<Option<ProfileStats>> {
        let query = "SELECT follower_count, following_count, post_count FROM profile_stats WHERE profile_id = ? AND region_code = ?";

        // Exécuter la requête
        let result = self.session
            .query_unpaged(query, (profile_id.as_uuid(), region.as_str().to_string()))
            .await
            .map_err(|e| DomainError::Infrastructure(e.to_string()))?;

        // Convertir le résultat en rows_result
        let rows_result = result
            .into_rows_result()
            .map_err(|e| DomainError::Infrastructure(format!("Result conversion error: {}", e)))?;

        // Obtenir un itérateur typé sur les rows
        let mut rows_iter = rows_result.rows::<(Counter, Counter, Counter)>().map_err(|e| DomainError::Infrastructure(e.to_string()))?;

        // Prendre la première row
        if let Some(row_result) = rows_iter.next() {
            // 2. On récupère les objets Counter
            let (followers_cnt, following_cnt, posts_cnt) = row_result.map_err(|e| DomainError::Infrastructure(e.to_string()))?;

            // 3. On extrait la valeur .0 qui est le i64 interne
            let stats = ProfileStats::new(
                followers_cnt.0.max(0) as u64,
                following_cnt.0.max(0) as u64,
                posts_cnt.0.max(0) as u64
            );

            return Ok(Some(stats));
        }

        Ok(None)
    }

    async fn save(
        &self,
        profile_id: &ProfileId,
        region: &RegionCode,
        follower_delta: i64,
        following_delta: i64,
        post_delta: i64,
    ) -> Result<()> {
        let query = "UPDATE profile_stats SET \
                     follower_count = follower_count + ?, \
                     following_count = following_count + ?, \
                     post_count = post_count + ? \
                     WHERE profile_id = ? AND region_code = ?";

        let values = (
            Counter(follower_delta),
            Counter(following_delta),
            Counter(post_delta),
            profile_id.as_uuid(),
            region.as_str().to_string(),
        );

        self.session
            .query_unpaged(query, values)
            .await
            .map_err(|e| DomainError::Infrastructure(e.to_string()))?;

        Ok(())
    }

    async fn delete(&self, profile_id: &ProfileId, region: &RegionCode) -> Result<()> {
        let prepared = self
            .session
            .prepare("DELETE FROM profile_stats WHERE profile_id = ? AND region_code = ?")
            .await
            .map_err(|e| DomainError::Infrastructure(e.to_string()))?;

        self.session
            .execute_unpaged(
                &prepared,
                (profile_id.as_uuid(), region.as_str().to_string()),
            )
            .await
            .map_err(|e| DomainError::Infrastructure(e.to_string()))?;
        Ok(())
    }
}
