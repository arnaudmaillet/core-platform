// crates/post/src/infrastructure/repositories/scylla_post_repository.rs

use async_trait::async_trait;
use infra_scylla::scylla::errors::PrepareError;
use infra_scylla::scylla::{
    client::session::Session, statement::prepared::PreparedStatement, value::CqlTimestamp,
};
use shared_kernel::core::{Error, Identifier, PageQuery, PagedResult, Result, Versioned};
use shared_kernel::types::{PostId, ProfileId, Region};
use std::sync::Arc;

use crate::domain::entities::Post;
use crate::mappers::{CqlMediaAsset, CqlPostRow};
use crate::repositories::PostRepository;

macro_rules! insert_author_cql {
    () => {
        "INSERT INTO {}.posts_by_author \
         (region, post_id, author_id, post_type, caption, media_list, total_duration_seconds, allowed_comment_hands, visibility_level, music_id, hashtags, mentions, is_edited, updated_at, dynamic_metadata) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
    };
}

macro_rules! insert_id_cql {
    () => {
        "INSERT INTO {}.posts_by_id \
         (region, post_id, author_id, post_type, caption, media_list, total_duration_seconds, allowed_comment_hands, visibility_level, music_id, hashtags, mentions, is_edited, updated_at, dynamic_metadata) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
    };
}

macro_rules! find_by_id_cql {
    () => {
        "SELECT * FROM {}.posts_by_id WHERE region = ? AND post_id = ? LIMIT 1"
    };
}

macro_rules! find_by_author_cql {
    () => {
        "SELECT * FROM {}.posts_by_author WHERE region = ? AND author_id = ?"
    };
}

macro_rules! delete_author_cql {
    () => {
        "DELETE FROM {}.posts_by_author WHERE region = ? AND author_id = ? AND post_id = ?"
    };
}

macro_rules! delete_id_cql {
    () => {
        "DELETE FROM {}.posts_by_id WHERE region = ? AND post_id = ?"
    };
}

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
    pub async fn new(
        session: Arc<Session>,
        keyspace: &str,
    ) -> std::result::Result<Self, PrepareError> {
        let insert_author_stmt = session
            .prepare(format!(insert_author_cql!(), keyspace))
            .await?;
        let insert_id_stmt = session.prepare(format!(insert_id_cql!(), keyspace)).await?;
        let find_by_id_stmt = session
            .prepare(format!(find_by_id_cql!(), keyspace))
            .await?;
        let find_by_author_stmt = session
            .prepare(format!(find_by_author_cql!(), keyspace))
            .await?;
        let delete_author_stmt = session
            .prepare(format!(delete_author_cql!(), keyspace))
            .await?;
        let delete_id_stmt = session.prepare(format!(delete_id_cql!(), keyspace)).await?;

        Ok(Self {
            session,
            insert_author_stmt,
            insert_id_stmt,
            find_by_id_stmt,
            find_by_author_stmt,
            delete_author_stmt,
            delete_id_stmt,
        })
    }
}

#[async_trait]
impl PostRepository for ScyllaPostRepository {
    async fn save(&self, region: Region, post: &Post) -> Result<()> {
        let region_str = region.to_string();
        let caption = post.caption().as_ref().map(|c| c.to_string());

        let cql_media: Vec<CqlMediaAsset> =
            post.media_list().iter().map(CqlMediaAsset::from).collect();

        let hashtags: std::collections::HashSet<String> =
            post.hashtags().value().iter().cloned().collect();

        let mentions: std::collections::HashSet<uuid::Uuid> = post
            .mentions()
            .value()
            .iter()
            .map(|id| id.as_uuid())
            .collect();

        let updated_at = Some(CqlTimestamp(post.updated_at().timestamp_millis()));

        let params = (
            &region_str,
            post.post_id().as_uuid(),
            post.author_id().as_uuid(),
            post.post_type().to_string(),
            caption,
            cql_media,
            post.total_duration_seconds() as i32,
            post.allowed_comment_hands(),
            post.visibility_level().to_string(),
            post.music_id().map(|m| m.uuid()),
            hashtags,
            mentions,
            post.is_edited(),
            updated_at,
            post.dynamic_metadata().to_string(),
        );

        // Écritures concurrentes via tokio::join! dans tes deux tables de requêtage (dual-write pattern)
        let fut_author = self
            .session
            .execute_unpaged(&self.insert_author_stmt, &params);
        let fut_id = self.session.execute_unpaged(&self.insert_id_stmt, &params);

        let (res_author, res_id) = tokio::join!(fut_author, fut_id);
        res_author.map_err(|e| Error::database(format!("Author write failed: {}", e)))?;
        res_id.map_err(|e| Error::database(format!("ID write failed: {}", e)))?;

        Ok(())
    }

    async fn find_by_id(&self, region: Region, post_id: &PostId) -> Result<Option<Post>> {
        let res = self
            .session
            .execute_unpaged(
                &self.find_by_id_stmt,
                (region.to_string(), post_id.as_uuid()),
            )
            .await
            .map_err(|e| Error::database(format!("Scylla find_by_id failed: {}", e)))?;

        let rows_result = res
            .into_rows_result()
            .map_err(|e| Error::database(format!("Invalid rows format: {}", e)))?;

        if let Some(row) = rows_result
            .maybe_first_row::<CqlPostRow>()
            .map_err(|e| Error::database(format!("Row parsing failed: {}", e)))?
        {
            let post = Post::try_from(row)?;
            Ok(Some(post))
        } else {
            Ok(None)
        }
    }

    async fn find_by_author(
        &self,
        region: Region,
        author_id: &ProfileId,
        query: PageQuery,
    ) -> Result<PagedResult<Post>> {
        let mut executable_stmt = self.find_by_author_stmt.clone();
        executable_stmt.set_page_size(query.limit as i32);

        let query_res = self
            .session
            .execute_unpaged(&executable_stmt, (region.to_string(), author_id.as_uuid()))
            .await
            .map_err(|e| Error::database(format!("Scylla find_by_author failed: {}", e)))?;

        let rows_result = query_res
            .into_rows_result()
            .map_err(|e| Error::database(format!("Invalid rows format: {}", e)))?;

        let rows_iter = rows_result
            .rows::<CqlPostRow>()
            .map_err(|e| Error::database(format!("Iterator failure: {}", e)))?;

        let mut posts = Vec::<Post>::new();

        for row_res in rows_iter {
            let cql_row: CqlPostRow =
                row_res.map_err(|e| Error::database(format!("Row parsing failed: {}", e)))?;
            posts.push(Post::try_from(cql_row)?);
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

    async fn delete(&self, region: Region, post_id: &PostId, author_id: &ProfileId) -> Result<()> {
        let region_str = region.to_string();

        let fut_author = self.session.execute_unpaged(
            &self.delete_author_stmt,
            (&region_str, author_id.as_uuid(), post_id.as_uuid()),
        );
        let fut_id = self
            .session
            .execute_unpaged(&self.delete_id_stmt, (&region_str, post_id.as_uuid()));

        let (res_author, res_id) = tokio::join!(fut_author, fut_id);
        res_author.map_err(|e| Error::database(format!("Author delete failed: {}", e)))?;
        res_id.map_err(|e| Error::database(format!("ID delete failed: {}", e)))?;

        Ok(())
    }
}
