// crates/post/src/infrastructure/repositories/scylla_post_repository.rs

use async_trait::async_trait;
use infra_scylla::scylla::errors::PrepareError;
use infra_scylla::scylla::{client::session::Session, statement::prepared::PreparedStatement};
use shared_kernel::core::{Error, Identifier, PageQuery, PagedResult, Result};
use shared_kernel::types::{PostId, ProfileId, Region};
use std::sync::Arc;

use crate::domain::entities::Post;
use crate::infrastructure::mappers::CqlPostRow;
use crate::repositories::PostRepository;

macro_rules! insert_author_cql {
    () => {
        "INSERT INTO {}.posts_by_author \
         (author_id, post_id, post_type, caption, media_list, total_duration_seconds, allowed_comment_hands, visibility_level, music_id, hashtags, mentions, version, edited_at, created_at, updated_at, dynamic_metadata) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
    };
}

macro_rules! insert_id_cql {
    () => {
        "INSERT INTO {}.posts_by_id \
         (post_id, author_id, post_type, caption, media_list, total_duration_seconds, allowed_comment_hands, visibility_level, music_id, hashtags, mentions, version, edited_at, created_at, updated_at, dynamic_metadata) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
    };
}

macro_rules! find_by_id_cql {
    () => {
        "SELECT * FROM {}.posts_by_id WHERE post_id = ? LIMIT 1"
    };
}

macro_rules! find_by_author_cql {
    () => {
        "SELECT * FROM {}.posts_by_author WHERE author_id = ?"
    };
}

macro_rules! delete_author_cql {
    () => {
        "DELETE FROM {}.posts_by_author WHERE author_id = ? AND post_id = ?"
    };
}

macro_rules! delete_id_cql {
    () => {
        "DELETE FROM {}.posts_by_id WHERE post_id = ?"
    };
}

pub struct ScyllaPostStore {
    session: Arc<Session>,
    insert_author_stmt: PreparedStatement,
    insert_id_stmt: PreparedStatement,
    find_by_id_stmt: PreparedStatement,
    find_by_author_stmt: PreparedStatement,
    delete_author_stmt: PreparedStatement,
    delete_id_stmt: PreparedStatement,
}

impl ScyllaPostStore {
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
impl PostRepository for ScyllaPostStore {
    async fn save(&self, _region: Region, post: &Post) -> Result<()> {
        let row = CqlPostRow::from_domain(post);

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

        let id_params = (
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

        let fut_author = self
            .session
            .execute_unpaged(&self.insert_author_stmt, &author_params);
        let fut_id = self
            .session
            .execute_unpaged(&self.insert_id_stmt, &id_params);

        let (res_author, res_id) = tokio::join!(fut_author, fut_id);
        res_author.map_err(|e| Error::database(format!("Author write failed: {}", e)))?;
        res_id.map_err(|e| Error::database(format!("ID write failed: {}", e)))?;

        Ok(())
    }

    async fn find_by_id(&self, _region: Region, post_id: &PostId) -> Result<Option<Post>> {
        let res = self
            .session
            .execute_unpaged(&self.find_by_id_stmt, (post_id.as_uuid(),))
            .await
            .map_err(|e| Error::database(format!("Scylla find_by_id failed: {}", e)))?;

        let rows_result = res
            .into_rows_result()
            .map_err(|e| Error::database(format!("Invalid rows format: {}", e)))?;

        if let Some(row) = rows_result
            .maybe_first_row::<CqlPostRow>()
            .map_err(|e| Error::database(format!("Row parsing failed: {}", e)))?
        {
            let post = row.to_domain()?;
            Ok(Some(post))
        } else {
            Ok(None)
        }
    }

    async fn find_by_author(
        &self,
        _region: Region,
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
            .rows::<CqlPostRow>()
            .map_err(|e| Error::database(format!("Iterator failure: {}", e)))?;

        let mut posts = Vec::<Post>::new();

        for row_res in rows_iter {
            let scylla_row: CqlPostRow =
                row_res.map_err(|e| Error::database(format!("Row parsing failed: {}", e)))?;
            posts.push(scylla_row.to_domain()?);
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

    async fn delete(&self, _region: Region, post_id: &PostId, author_id: &ProfileId) -> Result<()> {
        let fut_author = self.session.execute_unpaged(
            &self.delete_author_stmt,
            (author_id.as_uuid(), post_id.as_uuid()),
        );
        let fut_id = self
            .session
            .execute_unpaged(&self.delete_id_stmt, (post_id.as_uuid(),));

        let (res_author, res_id) = tokio::join!(fut_author, fut_id);
        res_author.map_err(|e| Error::database(format!("Author delete failed: {}", e)))?;
        res_id.map_err(|e| Error::database(format!("ID delete failed: {}", e)))?;

        Ok(())
    }
}
