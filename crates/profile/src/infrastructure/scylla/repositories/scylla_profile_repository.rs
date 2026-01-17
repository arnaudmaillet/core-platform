// crates/profile/src/infrastructure/repositories/scylla_profile_repository.rs

use async_trait::async_trait;
use std::sync::Arc;
use scylla::client::session::Session;
use scylla::value::Row;
use shared_kernel::domain::value_objects::{Counter, RegionCode, AccountId};
use shared_kernel::errors::{Result, DomainError};

use crate::domain::repositories::ProfileStatsRepository;
use crate::domain::value_objects::ProfileStats;

pub struct ScyllaProfileRepository {
    session: Arc<Session>,
}

impl ScyllaProfileRepository {
    pub fn new(session: Arc<Session>) -> Self {
        Self { session }
    }

    /// Helper function to parse a row into follower/following counts
    fn parse_profile_row(row: &Row) -> Result<(i64, i64)> {
        // Dans Scylla 1.4.1, les colonnes sont accessibles par position
        // Assurez-vous que l'index correspond à l'ordre des colonnes dans la requête SELECT
        let followers = row.columns
            .get(0)
            .and_then(|col| col.as_ref())
            .and_then(|c| c.as_bigint())
            .ok_or_else(|| DomainError::Infrastructure("Failed to parse follower_count".into()))?;

        let following = row.columns
            .get(1)
            .and_then(|col| col.as_ref())
            .and_then(|c| c.as_bigint())
            .ok_or_else(|| DomainError::Infrastructure("Failed to parse following_count".into()))?;

        Ok((followers, following))
    }
}

#[async_trait]
impl ProfileStatsRepository for ScyllaProfileRepository {
    async fn find_by_id(&self, account_id: &AccountId, region: &RegionCode) -> Result<Option<ProfileStats>> {
        let query = "SELECT follower_count, following_count FROM profile_stats WHERE account_id = ? AND region_code = ?";

        // Exécuter la requête
        let result = self.session
            .query_unpaged(query, (account_id.as_uuid(), region.as_str().to_string()))
            .await
            .map_err(|e| DomainError::Infrastructure(e.to_string()))?;

        // Convertir le résultat en rows_result
        let rows_result = result.into_rows_result()
            .map_err(|e| DomainError::Infrastructure(format!("Result conversion error: {}", e)))?;

        // Obtenir un itérateur typé sur les rows
        let mut rows_iter = rows_result.rows::<(i64, i64)>()
            .map_err(|e| DomainError::Infrastructure(format!("Type mapping error: {}", e)))?;

        // Prendre la première row
        if let Some(row_result) = rows_iter.next() {
            let (followers, following) = row_result
                .map_err(|e| DomainError::Infrastructure(format!("Row parsing error: {}", e)))?;

            let stats = ProfileStats {
                follower_count: Counter::try_from(followers)?,
                following_count: Counter::try_from(following)?,
            };

            return Ok(Some(stats));
        }

        Ok(None)
    }

    async fn update_stats(
        &self,
        account_id: &AccountId,
        region: &RegionCode,
        follower_delta: i64,
        following_delta: i64,
        post_delta: i64
    ) -> Result<()> {
        // En ScyllaDB, les compteurs se mettent à jour avec SET val = val + ?
        // Si le delta est négatif (ex: -1), Scylla gère la soustraction automatiquement.
        let query = "UPDATE profile_stats SET \
                     follower_count = follower_count + ?, \
                     following_count = following_count + ?, \
                     post_count = post_count + ? \
                     WHERE account_id = ? AND region_code = ?";

        let prepared = self.session
            .prepare(query)
            .await
            .map_err(|e| DomainError::Infrastructure(e.to_string()))?;

        self.session
            .execute_unpaged(&prepared, (
                follower_delta,
                following_delta,
                post_delta,
                account_id.as_uuid(),
                region.as_str().to_string()
            ))
            .await
            .map_err(|e| DomainError::Infrastructure(e.to_string()))?;

        Ok(())
    }

    async fn delete_stats(&self, account_id: &AccountId, region: &RegionCode) -> Result<()> {
        let prepared = self.session
            .prepare("DELETE FROM profile_stats WHERE account_id = ? AND region_code = ?")
            .await
            .map_err(|e| DomainError::Infrastructure(e.to_string()))?;

        self.session
            .execute_unpaged(&prepared, (account_id.as_uuid(), region.as_str().to_string()))
            .await
            .map_err(|e| DomainError::Infrastructure(e.to_string()))?;
        Ok(())
    }
}