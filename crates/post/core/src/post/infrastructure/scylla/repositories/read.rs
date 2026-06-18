// crates/post/core/src/post/infrastructure/scylla/read.rs

use crate::Post;
use crate::post::domain::repositories::PostReadRepository;
use crate::post::infrastructure::scylla::ScyllaPostModel;
use crate::post::infrastructure::scylla::statements::{FIND_POST_BY_ID, FIND_POSTS_BY_AUTHOR};
use async_trait::async_trait;
use infra_scylla::scylla::{client::session::Session, statement::prepared::PreparedStatement};
use shared_kernel::core::{Error, Identifier, PageQuery, PagedResult, Result};
use shared_kernel::types::{PostId, ProfileId};
use std::sync::Arc;

pub struct ScyllaPostReadRepository {
    session: Arc<Session>,
    find_by_id_stmt: PreparedStatement,
    find_by_author_stmt: PreparedStatement,
}

impl ScyllaPostReadRepository {
    pub async fn new(session: Arc<Session>, keyspace: String) -> Result<Self> {
        let prepare = |template: &'static str, ks: String, sess: Arc<Session>| async move {
            let cql = template.replace("{ks}", &ks);
            sess.prepare(cql).await.map_err(|e| {
                Error::database(format!(
                    "ScyllaDB Prepare failed for post read keyspace '{}': {}",
                    ks, e
                ))
            })
        };

        Ok(Self {
            find_by_id_stmt: prepare(FIND_POST_BY_ID, keyspace.clone(), session.clone()).await?,
            find_by_author_stmt: prepare(FIND_POSTS_BY_AUTHOR, keyspace.clone(), session.clone())
                .await?,
            session,
        })
    }
}

#[async_trait]
impl PostReadRepository for ScyllaPostReadRepository {
    async fn find_by_id(&self, post_id: &PostId) -> Result<Option<Post>> {
        let res = self
            .session
            .execute_unpaged(&self.find_by_id_stmt, (post_id.as_uuid(),))
            .await
            .map_err(|e| Error::database(format!("Scylla find_by_id failed: {}", e)))?;

        let rows_result = res
            .into_rows_result()
            .map_err(|e| Error::database(e.to_string()))?;

        if let Some(row) = rows_result
            .maybe_first_row::<ScyllaPostModel>()
            .map_err(|e| Error::database(e.to_string()))?
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
            .map_err(|e| Error::database(e.to_string()))?;
        let rows_iter = rows_result
            .rows::<ScyllaPostModel>()
            .map_err(|e| Error::database(e.to_string()))?;

        let mut posts = Vec::<Post>::new();
        for row_res in rows_iter {
            let scylla_row: ScyllaPostModel =
                row_res.map_err(|e| Error::database(e.to_string()))?;
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
}
