// crates/post/src/infrastructure/profile/scylla/projection.rs

use crate::application::repositories::ProfileProjectionRepository;
use crate::infrastructure::profile::scylla::statements::{
    FIND_PROFILE_PROJECTION_BY_ID, SAVE_PROFILE_PROJECTION,
};
use crate::infrastructure::profile::scylla::{ScyllaProfileModel, ScyllaProfileUpdateModel};
use async_trait::async_trait;
use infra_scylla::scylla::{client::session::Session, statement::prepared::PreparedStatement};
use shared_kernel::core::{Error, Identifier, Result};
use shared_kernel::types::ProfileId;
use shared_proto::profile::v1::ProfileSummaryDto;
use std::sync::Arc;

pub struct ScyllaProfileProjection {
    session: Arc<Session>,
    save_projection_stmt: PreparedStatement,
    find_projection_stmt: PreparedStatement,
}

impl ScyllaProfileProjection {
    pub async fn new(session: Arc<Session>, keyspace: String) -> Result<Self> {
        tracing::info!(
            "Preparing Scylla projection statements for keyspace: {}",
            keyspace
        );

        let prepare = |template: &'static str, ks: String, sess: Arc<Session>| async move {
            let cql = template.replace("{ks}", &ks);
            tracing::debug!("Preparing Projection CQL: {}", cql);
            sess.prepare(cql).await.map_err(|e| {
                Error::database(format!(
                    "ScyllaDB PreparedStatement failed for projection keyspace '{}': {}",
                    ks, e
                ))
            })
        };

        Ok(Self {
            save_projection_stmt: prepare(
                SAVE_PROFILE_PROJECTION,
                keyspace.clone(),
                session.clone(),
            )
            .await?,
            find_projection_stmt: prepare(
                FIND_PROFILE_PROJECTION_BY_ID,
                keyspace.clone(),
                session.clone(),
            )
            .await?,
            session,
        })
    }
}

#[async_trait]
impl ProfileProjectionRepository for ScyllaProfileProjection {
    async fn save(&self, profile: &ProfileSummaryDto, updated_at_ms: i64) -> Result<()> {
        // Utilisation du nouveau mapper dédié aux lifetimes d'écriture
        let update = ScyllaProfileUpdateModel::try_from_dto(profile)?;

        // Règle d'or de concurrence ScyllaDB : USING TIMESTAMP attend des microsecondes
        let scylla_internal_timestamp = updated_at_ms * 1000;

        let params = (
            update.profile_id,
            update.handle,
            update.display_name,
            update.avatar_url,
            update.is_verified,
            updated_at_ms,
            scylla_internal_timestamp,
        );

        self.session
            .execute_unpaged(&self.save_projection_stmt, params)
            .await
            .map_err(|e| Error::database(format!("Failed to save profile projection: {}", e)))?;

        Ok(())
    }

    async fn find_by_id(&self, profile_id: &ProfileId) -> Result<Option<ProfileSummaryDto>> {
        let res = self
            .session
            .execute_unpaged(&self.find_projection_stmt, (profile_id.as_uuid(),))
            .await
            .map_err(|e| {
                Error::database(format!("Failed to execute find profile projection: {}", e))
            })?;

        let rows_res = res
            .into_rows_result()
            .map_err(|e| Error::database(e.to_string()))?;

        if let Some(row) = rows_res
            .maybe_first_row::<ScyllaProfileModel>()
            .map_err(|e| Error::database(e.to_string()))?
        {
            return Ok(Some(ProfileSummaryDto::from(row)));
        }

        Ok(None)
    }
}
