// crates/post/src/infrastructure/post/scylla/repository.rs

use crate::domain::entities::Post;
use crate::infrastructure::post::ScyllaPostModel;
use crate::infrastructure::post::scylla::statements::{
    DELETE_POST_BY_AUTHOR, DELETE_POST_BY_ID, FIND_POST_BY_ID, FIND_POSTS_BY_AUTHOR,
    INSERT_POST_BY_AUTHOR, INSERT_POST_BY_ID,
};
use crate::repositories::PostRepository;
use async_trait::async_trait;
use infra_scylla::scylla::{client::session::Session, statement::prepared::PreparedStatement};
use shared_kernel::core::{Error, Identifier, PageQuery, PagedResult, Result};
use shared_kernel::types::{PostId, ProfileId};
use std::sync::Arc;

pub struct ScyllaPostRepository {
    session: Arc<Session>,
    insert_author_stmt: PreparedStatement,
    insert_id_stmt: PreparedStatement,
    find_by_id_stmt: PreparedStatement,
    find_by_author_stmt: PreparedStatement,
    delete_author_stmt: PreparedStatement,
    delete_id_stmt: PreparedStatement,
}

impl ScyllaPostRepository {
    pub async fn new(session: Arc<Session>, keyspace: String) -> Result<Self> {
        tracing::info!(
            "Preparing Scylla post statements for keyspace: {}",
            keyspace
        );

        let prepare = |template: &'static str, ks: String, sess: Arc<Session>| async move {
            let cql = template.replace("{ks}", &ks);
            tracing::debug!("Preparing Post CQL: {}", cql);
            sess.prepare(cql).await.map_err(|e| {
                Error::database(format!(
                    "ScyllaDB PreparedStatement failed for post keyspace '{}': {}",
                    ks, e
                ))
            })
        };

        Ok(Self {
            insert_author_stmt: prepare(INSERT_POST_BY_AUTHOR, keyspace.clone(), session.clone())
                .await?,
            insert_id_stmt: prepare(INSERT_POST_BY_ID, keyspace.clone(), session.clone()).await?,
            find_by_id_stmt: prepare(FIND_POST_BY_ID, keyspace.clone(), session.clone()).await?,
            find_by_author_stmt: prepare(FIND_POSTS_BY_AUTHOR, keyspace.clone(), session.clone())
                .await?,
            delete_author_stmt: prepare(DELETE_POST_BY_AUTHOR, keyspace.clone(), session.clone())
                .await?,
            delete_id_stmt: prepare(DELETE_POST_BY_ID, keyspace.clone(), session.clone()).await?,
            session,
        })
    }
}

#[async_trait]
impl PostRepository for ScyllaPostRepository {
    async fn save(&self, post: &Post) -> Result<()> {
        let row = ScyllaPostModel::from(post);

        // On extrait les paramètres communs de manière partagée pour éviter les allocations en double
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
            row.version,
            row.edited_at,
            row.created_at,
            row.updated_at,
            &row.dynamic_metadata,
        );

        // Pour posts_by_author, l'ordre des PK est (author_id, post_id)
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
            row.version,
            row.edited_at,
            row.created_at,
            row.updated_at,
            &row.dynamic_metadata,
        );

        // Exécution simultanée optimisée (Pattern Scatter-Gather local)
        let fut_author = self
            .session
            .execute_unpaged(&self.insert_author_stmt, author_params);
        let fut_id = self.session.execute_unpaged(&self.insert_id_stmt, params);

        // try_join court-circuite immédiatement si l'une des deux écritures crash
        tokio::try_join!(fut_author, fut_id).map_err(|e| {
            Error::database(format!(
                "Hyperscale dual-write post replication failed: {}",
                e
            ))
        })?;

        Ok(())
    }

    async fn find_by_id(&self, post_id: &PostId) -> Result<Option<Post>> {
        let res = self
            .session
            .execute_unpaged(&self.find_by_id_stmt, (post_id.as_uuid(),))
            .await
            .map_err(|e| Error::database(format!("Scylla find_by_id failed: {}", e)))?;

        let rows_result = res
            .into_rows_result()
            .map_err(|e| Error::database(format!("Invalid rows format: {}", e)))?;

        if let Some(row) = rows_result
            .maybe_first_row::<ScyllaPostModel>()
            .map_err(|e| Error::database(format!("Row parsing failed: {}", e)))?
        {
            let domain_post = Post::try_from(row)?;
            Ok(Some(domain_post))
        } else {
            Ok(None)
        }
    }

    async fn find_by_author(
        &self,
        author_id: &ProfileId,
        query: PageQuery,
    ) -> Result<PagedResult<Post>> {
        let mut executable_stmt = self.find_by_author_stmt.clone();
        executable_stmt.set_page_size(query.limit as i32);

        let query_res = self
            .session
            .execute_unpaged(&executable_stmt, (author_id.as_uuid(),))
            .await
            .map_err(|e| Error::database(format!("Scylla find_by_author failed: {}", e)))?;

        let rows_result = query_res
            .into_rows_result()
            .map_err(|e| Error::database(format!("Invalid rows format: {}", e)))?;

        let rows_iter = rows_result
            .rows::<ScyllaPostModel>()
            .map_err(|e| Error::database(format!("Iterator failure: {}", e)))?;

        let mut posts = Vec::<Post>::new();
        for row_res in rows_iter {
            let scylla_row: ScyllaPostModel =
                row_res.map_err(|e| Error::database(format!("Row parsing failed: {}", e)))?;
            posts.push(Post::try_from(scylla_row)?);
        }

        let total_found = posts.len();
        let has_more = total_found >= query.limit;

        if has_more {
            posts.truncate(query.limit);
        }

        let next_cursor = if has_more {
            query
                .cursor
                .map(|c| format!("{}_next", c))
                .or(Some("page_1".to_string()))
        } else {
            None
        };

        Ok(PagedResult {
            items: posts,
            next_cursor,
        })
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
