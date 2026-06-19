// crates/post/core/src/post/infrastructure/scylla/write.rs

use crate::Post;
use crate::post::domain::repositories::PostWriteRepository;
use crate::post::infrastructure::scylla::ScyllaPostModel;
use crate::post::infrastructure::scylla::statements::{
    DELETE_POST_BY_AUTHOR, DELETE_POST_BY_ID, INSERT_POST_BY_AUTHOR, INSERT_POST_BY_ID,
};
use async_trait::async_trait;
use infra_scylla::scylla::{client::session::Session, statement::prepared::PreparedStatement};
use shared_kernel::core::{Error, Identifier, Result, Versioned};
use shared_kernel::types::{PostId, ProfileId};
use std::sync::Arc;

pub struct ScyllaPostWriteRepository {
    session: Arc<Session>,
    insert_author_stmt: PreparedStatement,
    insert_id_stmt: PreparedStatement,
    delete_author_stmt: PreparedStatement,
    delete_id_stmt: PreparedStatement,
}

impl ScyllaPostWriteRepository {
    pub async fn new(session: Arc<Session>, keyspace: String) -> Result<Self> {
        let prepare = |template: &'static str, ks: String, sess: Arc<Session>| async move {
            let cql = template.replace("{ks}", &ks);
            sess.prepare(cql).await.map_err(|e| {
                Error::database(format!(
                    "ScyllaDB Prepare failed for post write keyspace '{}': {}",
                    ks, e
                ))
            })
        };

        Ok(Self {
            insert_author_stmt: prepare(INSERT_POST_BY_AUTHOR, keyspace.clone(), session.clone())
                .await?,
            insert_id_stmt: prepare(INSERT_POST_BY_ID, keyspace.clone(), session.clone()).await?,
            delete_author_stmt: prepare(DELETE_POST_BY_AUTHOR, keyspace.clone(), session.clone())
                .await?,
            delete_id_stmt: prepare(DELETE_POST_BY_ID, keyspace.clone(), session.clone()).await?,
            session,
        })
    }
}

#[async_trait]
impl PostWriteRepository for ScyllaPostWriteRepository {
    async fn save(&self, post: &Post) -> Result<()> {
        let row = ScyllaPostModel::from(post);

        // Race conditions guard, comparing exact timestamp
        let client_timestamp_micros = post.updated_at().timestamp_micros();

        let params = (
            row.post_id,
            row.author_id,
            &row.post_type,
            &row.caption,
            &row.media_list,
            row.total_duration_seconds,
            row.allowed_comment_hands,
            &row.visibility_level,
            row.music_id,
            &row.hashtags,
            &row.mentions,
            row.edited_at,
            row.created_at,
            &row.dynamic_metadata,
            client_timestamp_micros,
        );

        let author_params = (
            row.author_id,
            row.post_id,
            &row.post_type,
            &row.caption,
            &row.media_list,
            row.total_duration_seconds,
            row.allowed_comment_hands,
            &row.visibility_level,
            row.music_id,
            &row.hashtags,
            &row.mentions,
            row.edited_at,
            row.created_at,
            &row.dynamic_metadata,
            client_timestamp_micros,
        );

        let fut_author = self
            .session
            .execute_unpaged(&self.insert_author_stmt, author_params);
        let fut_id = self.session.execute_unpaged(&self.insert_id_stmt, params);

        tokio::try_join!(fut_author, fut_id)
            .map_err(|e| Error::database(format!("Dual-write post replication failed: {}", e)))?;

        Ok(())
    }

    async fn delete(&self, post_id: &PostId, author_id: &ProfileId) -> Result<()> {
        let fut_author = self.session.execute_unpaged(
            &self.delete_author_stmt,
            (author_id.as_uuid(), post_id.as_uuid()),
        );
        let fut_id = self
            .session
            .execute_unpaged(&self.delete_id_stmt, (post_id.as_uuid(),));

        tokio::try_join!(fut_author, fut_id)
            .map_err(|e| Error::database(format!("Atomic dual-delete post failed: {}", e)))?;

        Ok(())
    }
}
