// crates/post/core/src/post/domain/stubs/read_stub.rs

use async_trait::async_trait;
use crate::{Post, PostReadRepository};
use shared_kernel::core::{PageQuery, PagedResult, Result};
use shared_kernel::types::{PostId, ProfileId};
use std::collections::HashMap;
use std::sync::RwLock;

pub struct PostReadRepositoryStub {
    posts: RwLock<HashMap<PostId, Post>>,
}

impl PostReadRepositoryStub {
    pub fn new() -> Self {
        Self {
            posts: RwLock::new(HashMap::new()),
        }
    }

    pub fn feed(&self, post: Post) {
        let mut posts = self.posts.write().unwrap();
        posts.insert(post.post_id().clone(), post);
    }
}

#[async_trait]
impl PostReadRepository for PostReadRepositoryStub {
    async fn find_by_id(&self, post_id: &PostId) -> Result<Option<Post>> {
        let posts = self.posts.read().unwrap();
        Ok(posts.get(post_id).cloned())
    }

    async fn find_by_author(
        &self,
        author_id: &ProfileId,
        query: PageQuery,
    ) -> Result<PagedResult<Post>> {
        let posts = self.posts.read().unwrap();

        let mut author_posts: Vec<Post> = posts
            .values()
            .filter(|p| &p.author_id() == author_id)
            .cloned()
            .collect();

        author_posts.sort_by(|a, b| b.created_at().cmp(&a.created_at()));

        let total_found = author_posts.len();
        let has_more = total_found > query.limit;

        if has_more {
            author_posts.truncate(query.limit);
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
            items: author_posts,
            next_cursor,
        })
    }
}
