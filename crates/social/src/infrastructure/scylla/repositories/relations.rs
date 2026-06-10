// crates/social/src/infrastructure/scylla/repositories/relations.rs

use async_trait::async_trait;
use infra_scylla::scylla::{
    client::session::Session,
    statement::{
        batch::{Batch, BatchType},
        prepared::PreparedStatement,
    },
    value::CqlTimestamp,
};
use shared_kernel::{
    core::{Error, Identifier, Result},
    types::ProfileId,
};
use std::sync::Arc;

use crate::{domain::entities::FollowRelation, repositories::RelationRepository};

pub struct ScyllaRelationRepository {
    session: Arc<Session>,
    insert_following_stmt: PreparedStatement,
    insert_follower_stmt: PreparedStatement,
    delete_following_stmt: PreparedStatement,
    delete_follower_stmt: PreparedStatement,
    find_stmt: PreparedStatement,
    is_following_stmt: PreparedStatement,
    get_following_stmt: PreparedStatement,
    get_followers_stmt: PreparedStatement,
}

impl ScyllaRelationRepository {
    pub async fn new(session: Arc<Session>) -> Result<Self> {
        let insert_following_stmt = session
            .prepare(
                "INSERT INTO followings (follower_id, following_id, created_at) VALUES (?, ?, ?)",
            )
            .await
            .map_err(|e| Error::internal(format!("Prepare failed: {}", e)))?;

        let insert_follower_stmt = session
            .prepare(
                "INSERT INTO followers (following_id, follower_id, created_at) VALUES (?, ?, ?)",
            )
            .await
            .map_err(|e| Error::internal(format!("Prepare failed: {}", e)))?;

        let delete_following_stmt = session
            .prepare("DELETE FROM followings WHERE follower_id = ? AND following_id = ?")
            .await
            .map_err(|e| Error::internal(format!("Prepare failed: {}", e)))?;

        let delete_follower_stmt = session
            .prepare("DELETE FROM followers WHERE following_id = ? AND follower_id = ?")
            .await
            .map_err(|e| Error::internal(format!("Prepare failed: {}", e)))?;

        let find_stmt = session.prepare("SELECT created_at FROM followings WHERE follower_id = ? AND following_id = ? LIMIT 1")
            .await.map_err(|e| Error::internal(format!("Prepare failed: {}", e)))?;

        let is_following_stmt = session.prepare("SELECT follower_id FROM followings WHERE follower_id = ? AND following_id = ? LIMIT 1")
            .await.map_err(|e| Error::internal(format!("Prepare failed: {}", e)))?;

        let get_following_stmt = session
            .prepare("SELECT following_id FROM followings WHERE follower_id = ? LIMIT ?")
            .await
            .map_err(|e| Error::internal(format!("Prepare failed: {}", e)))?;

        let get_followers_stmt = session
            .prepare("SELECT follower_id FROM followers WHERE following_id = ? LIMIT ?")
            .await
            .map_err(|e| Error::internal(format!("Prepare failed: {}", e)))?;

        Ok(Self {
            session,
            insert_following_stmt,
            insert_follower_stmt,
            delete_following_stmt,
            delete_follower_stmt,
            find_stmt,
            is_following_stmt,
            get_following_stmt,
            get_followers_stmt,
        })
    }
}

#[async_trait]
impl RelationRepository for ScyllaRelationRepository {
    async fn save(&self, relation: &mut FollowRelation) -> Result<()> {
        let mut batch = Batch::new(BatchType::Logged);

        batch.append_statement(self.insert_following_stmt.clone());
        batch.append_statement(self.insert_follower_stmt.clone());

        let follower_uuid = relation.follower_id().as_uuid();
        let following_uuid = relation.following_id().as_uuid();
        let created_at_cql = CqlTimestamp(relation.created_at().timestamp_millis());

        let values = (
            (follower_uuid, following_uuid, created_at_cql),
            (following_uuid, follower_uuid, created_at_cql),
        );

        self.session
            .batch(&batch, values)
            .await
            .map_err(|e| Error::internal(format!("ScyllaDB Batch Save Failed: {}", e)))?;

        Ok(())
    }

    async fn delete(&self, relation: &mut FollowRelation) -> Result<()> {
        let follower_id = relation.follower_id();
        let following_id = relation.following_id();

        let mut batch = Batch::new(BatchType::Logged);
        batch.append_statement(self.delete_following_stmt.clone());
        batch.append_statement(self.delete_follower_stmt.clone());

        let values = (
            (follower_id.as_uuid(), following_id.as_uuid()),
            (following_id.as_uuid(), follower_id.as_uuid()),
        );

        self.session
            .batch(&batch, values)
            .await
            .map_err(|e| Error::internal(format!("ScyllaDB Batch Delete Failed: {}", e)))?;

        Ok(())
    }

    async fn find(
        &self,
        follower_id: ProfileId,
        following_id: ProfileId,
    ) -> Result<Option<FollowRelation>> {
        let res = self
            .session
            .execute_unpaged(
                &self.find_stmt,
                (follower_id.as_uuid(), following_id.as_uuid()),
            )
            .await
            .map_err(|e| Error::internal(e.to_string()))?;

        let rows_result = res
            .into_rows_result()
            .map_err(|e| Error::internal(format!("Invalid rows result: {}", e)))?;

        if let Some((cql_created_at,)) = rows_result
            .maybe_first_row::<(CqlTimestamp,)>()
            .map_err(|e| Error::internal(format!("Mapping error: {}", e)))?
        {
            let created_at = chrono::DateTime::from_timestamp_millis(cql_created_at.0)
                .ok_or_else(|| Error::internal("Invalid timestamp from ScyllaDB"))?;

            let relation =
                FollowRelation::restore(follower_id, following_id, created_at, chrono::Utc::now());
            return Ok(Some(relation));
        }

        Ok(None)
    }

    async fn is_following(&self, follower_id: ProfileId, following_id: ProfileId) -> Result<bool> {
        let res = self
            .session
            .execute_unpaged(
                &self.is_following_stmt,
                (follower_id.as_uuid(), following_id.as_uuid()),
            )
            .await
            .map_err(|e| Error::internal(e.to_string()))?;

        let rows_result = res
            .into_rows_result()
            .map_err(|e| Error::internal(format!("Invalid rows result: {}", e)))?;

        Ok(rows_result.rows_num() > 0)
    }

    async fn get_following_ids(
        &self,
        follower_id: ProfileId,
        limit: u32,
        _offset: u32,
    ) -> Result<Vec<ProfileId>> {
        let res = self
            .session
            .execute_unpaged(
                &self.get_following_stmt,
                (follower_id.as_uuid(), limit as i32),
            )
            .await
            .map_err(|e| Error::internal(e.to_string()))?;

        let rows_result = res
            .into_rows_result()
            .map_err(|e| Error::internal(format!("Invalid rows result: {}", e)))?;

        let mut ids = Vec::new();
        let mut rows_iter = rows_result
            .rows::<(uuid::Uuid,)>()
            .map_err(|e| Error::internal(format!("Iterator serialization failed: {}", e)))?;

        while let Some(row_result) = rows_iter.next() {
            let (following_uuid,) =
                row_result.map_err(|e| Error::internal(format!("Row mapping failed: {}", e)))?;

            ids.push(ProfileId::try_new(following_uuid.to_string())?);
        }

        Ok(ids)
    }

    async fn get_followers_ids(
        &self,
        following_id: ProfileId,
        limit: u32,
        _offset: u32,
    ) -> Result<Vec<ProfileId>> {
        let res = self
            .session
            .execute_unpaged(
                &self.get_followers_stmt,
                (following_id.as_uuid(), limit as i32),
            )
            .await
            .map_err(|e| Error::internal(e.to_string()))?;

        let rows_result = res
            .into_rows_result()
            .map_err(|e| Error::internal(format!("Invalid rows result: {}", e)))?;

        let mut ids = Vec::new();
        let mut rows_iter = rows_result
            .rows::<(uuid::Uuid,)>()
            .map_err(|e| Error::internal(format!("Iterator serialization failed: {}", e)))?;

        while let Some(row_result) = rows_iter.next() {
            let (follower_uuid,) =
                row_result.map_err(|e| Error::internal(format!("Row mapping failed: {}", e)))?;

            ids.push(ProfileId::try_new(follower_uuid.to_string())?);
        }

        Ok(ids)
    }
}
