// crates/profile/src/infrastructure/scylla/stores/routing_store.rs

use async_trait::async_trait;
use infra_scylla::scylla::{client::session::Session, statement::prepared::PreparedStatement};
use shared_kernel::core::{Error, Identifier, Result};
use shared_kernel::types::{ProfileId, Region};
use std::sync::Arc;
use uuid::Uuid;

use crate::infrastructure::scylla::mappers::{CqlRouterProfileRow, CqlRouterSlugRow};
use crate::infrastructure::scylla::statements::{
    DELETE_ROUTING_PROFILE, DELETE_ROUTING_SLUG, FIND_REGION_BY_ID, FIND_ROUTING_BY_SLUG,
    INSERT_ROUTING_PROFILE, INSERT_ROUTING_SLUG,
};
use crate::repositories::ProfileRoutingRepository;

pub struct ScyllaProfileRoutingStore {
    session: Arc<Session>,
    insert_profile_stmt: PreparedStatement,
    insert_slug_stmt: PreparedStatement,
    find_by_id_stmt: PreparedStatement,
    find_by_slug_stmt: PreparedStatement,
    delete_profile_stmt: PreparedStatement,
    delete_slug_stmt: PreparedStatement,
}

impl ScyllaProfileRoutingStore {
    pub async fn new(session: Arc<Session>) -> Result<Self> {
        let prep = |cql: &'static str, s: Arc<Session>| async move {
            s.prepare(cql)
                .await
                .map_err(|e| Error::database(format!("ScyllaDB PreparedStatement failed: {}", e)))
        };

        Ok(Self {
            insert_profile_stmt: prep(INSERT_ROUTING_PROFILE, session.clone()).await?,
            insert_slug_stmt: prep(INSERT_ROUTING_SLUG, session.clone()).await?,
            find_by_id_stmt: prep(FIND_REGION_BY_ID, session.clone()).await?,
            find_by_slug_stmt: prep(FIND_ROUTING_BY_SLUG, session.clone()).await?,
            delete_profile_stmt: prep(DELETE_ROUTING_PROFILE, session.clone()).await?,
            delete_slug_stmt: prep(DELETE_ROUTING_SLUG, session.clone()).await?,
            session,
        })
    }

    async fn try_reserve_slug(
        &self,
        slug_hash: &str,
        profile_id: Uuid,
        region_str: &str,
    ) -> Result<()> {
        let query_res = self
            .session
            .execute_unpaged(&self.insert_slug_stmt, (slug_hash, profile_id, region_str))
            .await
            .map_err(|e| Error::database(format!("Failed to execute slug LWT: {}", e)))?;

        let rows_res = query_res
            .into_rows_result()
            .map_err(|e| Error::database(e.to_string()))?;

        let mut rows_iter = rows_res
            .rows::<infra_scylla::scylla::value::Row>()
            .map_err(|e| Error::database(e.to_string()))?;

        if let Some(row_res) = rows_iter.next() {
            let brute_row = row_res.map_err(|e| Error::database(e.to_string()))?;

            let applied = brute_row.columns[0]
                .as_ref()
                .and_then(|c| c.as_boolean())
                .unwrap_or(false);

            if !applied {
                return Err(Error::concurrency_conflict(format!(
                    "The handle hash '{}' is already taken globally",
                    slug_hash
                )));
            }
            Ok(())
        } else {
            Err(Error::database(
                "Empty response from global routing LWT".to_string(),
            ))
        }
    }
}

#[async_trait]

impl ProfileRoutingRepository for ScyllaProfileRoutingStore {
    /// Tente de réserver un slug mondialement et d'enregistrer le routage associé.
    async fn register_routing(
        &self,
        profile_id: ProfileId,
        slug_hash: &str,
        region: Region,
    ) -> Result<()> {
        let pid = profile_id.as_uuid();
        let region_str = region.to_string();

        // Réutilisation de la fonction privée
        self.try_reserve_slug(slug_hash, pid, &region_str).await?;

        self.session
            .execute_unpaged(&self.insert_profile_stmt, (pid, &region_str))
            .await
            .map_err(|e| Error::database(format!("Failed to insert profile routing map: {}", e)))?;

        Ok(())
    }

    /// Retrouve la région d'un profil à partir de son ID ($O(1)$ global)
    async fn find_region_by_id(&self, profile_id: &ProfileId) -> Result<Option<Region>> {
        let res = self
            .session
            .execute_unpaged(&self.find_by_id_stmt, (profile_id.as_uuid(),))
            .await
            .map_err(|e| Error::database(e.to_string()))?;

        let rows = res
            .into_rows_result()
            .map_err(|e| Error::database(e.to_string()))?;

        if let Some(row) = rows
            .maybe_first_row::<CqlRouterProfileRow>()
            .map_err(|e| Error::database(e.to_string()))?
        {
            let region = Region::try_from(row.region.as_str())
                .map_err(|_| Error::internal("Invalid region stored in routing"))?;
            return Ok(Some(region));
        }
        Ok(None)
    }

    /// Retrouve l'ID et la région d'un profil à partir du hash du slug ($O(1)$ global)
    async fn resolve_slug(&self, slug_hash: &str) -> Result<Option<(ProfileId, Region)>> {
        let res = self
            .session
            .execute_unpaged(&self.find_by_slug_stmt, (slug_hash,))
            .await
            .map_err(|e| Error::database(e.to_string()))?;

        let rows = res
            .into_rows_result()
            .map_err(|e| Error::database(e.to_string()))?;

        if let Some(row) = rows
            .maybe_first_row::<CqlRouterSlugRow>()
            .map_err(|e| Error::database(e.to_string()))?
        {
            let region = Region::try_from(row.region.as_str())
                .map_err(|_| Error::internal("Invalid region stored in routing"))?;
            return Ok(Some((ProfileId::from_uuid(row.profile_id), region)));
        }
        Ok(None)
    }

    async fn update_slug_routing(
        &self,
        profile_id: ProfileId,
        old_slug_hash: &str,
        new_slug_hash: &str,
        region: Region,
    ) -> Result<()> {
        let pid = profile_id.as_uuid();
        let region_str = region.to_string();

        self.try_reserve_slug(new_slug_hash, pid, &region_str)
            .await?;

        self.session
            .execute_unpaged(&self.insert_profile_stmt, (pid, &region_str))
            .await
            .map_err(|e| Error::database(format!("Failed to update profile routing map: {}", e)))?;

        self.session
            .execute_unpaged(&self.delete_slug_stmt, (old_slug_hash,))
            .await
            .map_err(|e| Error::database(format!("Failed to release old slug routing: {}", e)))?;

        Ok(())
    }

    /// Supprime proprement les entrées de routage (ex: suppression de compte)
    async fn delete_routing(&self, profile_id: ProfileId, slug_hash: &str) -> Result<()> {
        self.session
            .execute_unpaged(&self.delete_slug_stmt, (slug_hash,))
            .await
            .map_err(|e| Error::database(format!("Failed to delete slug routing: {}", e)))?;

        self.session
            .execute_unpaged(&self.delete_profile_stmt, (profile_id.as_uuid(),))
            .await
            .map_err(|e| Error::database(format!("Failed to delete profile routing: {}", e)))?;

        Ok(())
    }
}
