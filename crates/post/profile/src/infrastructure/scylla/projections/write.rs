// crates/post/profile/src/infrastructure/scylla/write_projection.rs

use crate::infrastructure::scylla::ScyllaProfileUpdateModel;
use crate::infrastructure::scylla::statements::SAVE_PROFILE_PROJECTION;
use crate::{ProfileWriteProjection, ProjectedProfile};
use async_trait::async_trait;
use infra_scylla::scylla::{client::session::Session, statement::prepared::PreparedStatement};
use shared_kernel::core::{Error, Result};
use std::sync::Arc;

pub struct ScyllaProfileWriteProjection {
    session: Arc<Session>,
    save_projection_stmt: PreparedStatement,
}

impl ScyllaProfileWriteProjection {
    pub async fn new(session: Arc<Session>, keyspace: String) -> Result<Self> {
        let cql = SAVE_PROFILE_PROJECTION.replace("{ks}", &keyspace);
        let save_projection_stmt = session.prepare(cql).await.map_err(|e| {
            Error::database(format!(
                "ScyllaDB Prepare failed for write keyspace '{}': {}",
                keyspace, e
            ))
        })?;

        Ok(Self {
            session,
            save_projection_stmt,
        })
    }
}

#[async_trait]
impl ProfileWriteProjection for ScyllaProfileWriteProjection {
    async fn save(&self, profile: &ProjectedProfile, updated_at_ms: i64) -> Result<()> {
        let update = ScyllaProfileUpdateModel::from(profile);
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
            .map_err(|e| Error::database(format!("ScyllaDB save failed: {}", e)))?;

        Ok(())
    }
}
