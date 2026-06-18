// crates/post/profile/src/infrastructure/scylla/read_projection.rs

use crate::infrastructure::scylla::ScyllaProfileModel;
use crate::infrastructure::scylla::statements::FIND_PROFILE_PROJECTION_BY_ID;
use crate::{ProfileReadProjection, ProjectedProfile};
use async_trait::async_trait;
use infra_scylla::scylla::{client::session::Session, statement::prepared::PreparedStatement};
use shared_kernel::core::{Error, Identifier, Result};
use shared_kernel::types::ProfileId;
use std::sync::Arc;

pub struct ScyllaProfileReadProjection {
    session: Arc<Session>,
    find_projection_stmt: PreparedStatement,
}

impl ScyllaProfileReadProjection {
    pub async fn new(session: Arc<Session>, keyspace: String) -> Result<Self> {
        let cql = FIND_PROFILE_PROJECTION_BY_ID.replace("{ks}", &keyspace);
        let find_projection_stmt = session.prepare(cql).await.map_err(|e| {
            Error::database(format!(
                "ScyllaDB Prepare failed for read keyspace '{}': {}",
                keyspace, e
            ))
        })?;

        Ok(Self {
            session,
            find_projection_stmt,
        })
    }
}

#[async_trait]
impl ProfileReadProjection for ScyllaProfileReadProjection {
    async fn find_by_id(&self, profile_id: &ProfileId) -> Result<Option<ProjectedProfile>> {
        let res = self
            .session
            .execute_unpaged(&self.find_projection_stmt, (profile_id.as_uuid(),))
            .await
            .map_err(|e| Error::database(format!("ScyllaDB find_by_id failed: {}", e)))?;

        let rows_res = res
            .into_rows_result()
            .map_err(|e| Error::database(e.to_string()))?;

        if let Some(row) = rows_res
            .maybe_first_row::<ScyllaProfileModel>()
            .map_err(|e| Error::database(e.to_string()))?
        {
            return Ok(Some(ProjectedProfile {
                id: profile_id.clone(),
                handle: row.handle,
                display_name: row.display_name,
                avatar_url: row.avatar_url,
                is_verified: row.is_verified,
            }));
        }

        Ok(None)
    }
}
