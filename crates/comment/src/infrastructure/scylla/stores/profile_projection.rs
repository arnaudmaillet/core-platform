// crates/content_comments/src/infrastructure/scylla/profile_repository.rs

use async_trait::async_trait;
use chrono::Utc;
use infra_scylla::scylla::client::session::Session;
use infra_scylla::scylla::statement::prepared::PreparedStatement;
use std::collections::HashMap;
use std::sync::Arc;

use shared_kernel::core::{Error, Result};
use shared_kernel::types::ProfileId;

use crate::infrastructure::statements::{FIND_PROFILES_BATCH, INSERT_PROFILE};
use crate::repositories::CommentUserProfileRepository;
use crate::types::CommentUserProfile;

pub struct ScyllaCommentProfileStore {
    session: Arc<Session>,
    insert_stmt: PreparedStatement,
    find_batch_stmt: PreparedStatement,
}

impl ScyllaCommentProfileStore {
    pub async fn new(session: Arc<Session>) -> Result<Self> {
        let insert_stmt = session.prepare(INSERT_PROFILE).await.map_err(|e| {
            Error::internal(format!("ScyllaDB statement preparation failure: {}", e))
        })?;

        let find_batch_stmt = session.prepare(FIND_PROFILES_BATCH).await.map_err(|e| {
            Error::internal(format!("ScyllaDB statement preparation failure: {}", e))
        })?;

        Ok(Self {
            session,
            insert_stmt,
            find_batch_stmt,
        })
    }
}

#[async_trait]
impl CommentUserProfileRepository for ScyllaCommentProfileStore {
    async fn save(&self, profile: &CommentUserProfile) -> Result<()> {
        self.session
            .execute_unpaged(
                &self.insert_stmt,
                (
                    profile.profile_id().uuid(),
                    profile.username(),
                    profile.display_name(),
                    profile.avatar_url(),
                    Utc::now().timestamp_millis(),
                ),
            )
            .await
            .map_err(|e| Error::internal(format!("ScyllaDB save profile error: {}", e)))?;

        Ok(())
    }

    async fn save_batch(&self, profiles: Vec<CommentUserProfile>) -> Result<()> {
        if profiles.is_empty() {
            return Ok(());
        }

        let mut tasks = Vec::with_capacity(profiles.len());
        for profile in profiles {
            let session = self.session.clone();
            let stmt = self.insert_stmt.clone();
            tasks.push(tokio::spawn(async move {
                session
                    .execute_unpaged(
                        &stmt,
                        (
                            profile.profile_id().uuid(),
                            profile.username(),
                            profile.display_name(),
                            profile.avatar_url(),
                            Utc::now().timestamp_millis(),
                        ),
                    )
                    .await
            }));
        }

        for task in tasks {
            task.await
                .map_err(|_| Error::internal("Batch insert task panicked"))?
                .map_err(|e| Error::internal(format!("Batch insert execution failure: {}", e)))?;
        }

        Ok(())
    }

    async fn find_batch(
        &self,
        profile_ids: &[ProfileId],
    ) -> Result<HashMap<ProfileId, CommentUserProfile>> {
        if profile_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let uuids: Vec<uuid::Uuid> = profile_ids.iter().map(|id| id.uuid()).collect();

        let res = self
            .session
            .execute_unpaged(&self.find_batch_stmt, (uuids,))
            .await
            .map_err(|e| Error::internal(format!("ScyllaDB find_batch profiles error: {}", e)))?;

        let rows_res = res
            .into_rows_result()
            .map_err(|e| Error::internal(format!("ScyllaDB rows conversion error: {}", e)))?;

        type CqlProfileRow = (uuid::Uuid, String, String, Option<String>);
        let mut rows_iter = rows_res
            .rows::<CqlProfileRow>()
            .map_err(|e| Error::internal(format!("Scylla rows parsing error: {}", e)))?;

        let mut profiles_map = HashMap::new();

        while let Some(row_res) = rows_iter.next() {
            let (profile_uuid, username, display_name, avatar_url) = row_res
                .map_err(|e| Error::internal(format!("Scylla row fetching error: {}", e)))?;

            let profile_id = ProfileId::from(profile_uuid);
            let profile = CommentUserProfile::new(profile_id, username, display_name, avatar_url)?;

            profiles_map.insert(profile_id, profile);
        }

        Ok(profiles_map)
    }
}
