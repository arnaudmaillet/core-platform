// crates/post/src/infrastructure/repositories/post_repository_stub.rs

use async_trait::async_trait;
use shared_kernel::core::{PageQuery, PagedResult, Result};
use shared_kernel::types::{PostId, ProfileId, Region};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use post::entities::Post;
use post::repositories::PostRepository;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct RegionKey<I> {
    region: String,
    id: I,
}

#[derive(Default)]
struct InnerStorage {
    posts_by_id: HashMap<RegionKey<PostId>, Post>,
    posts_by_author: HashMap<RegionKey<ProfileId>, Vec<Post>>,
}

#[derive(Clone, Default)]
pub struct PostRepositoryStub {
    storage: Arc<RwLock<InnerStorage>>,
}

impl PostRepositoryStub {
    pub fn new() -> Self {
        Self {
            storage: Arc::new(RwLock::new(InnerStorage::default())),
        }
    }

    pub async fn count_all(&self) -> usize {
        let guard = self.storage.read().await;
        guard.posts_by_id.len()
    }
}

#[async_trait]
impl PostRepository for PostRepositoryStub {
    async fn save(&self, region: Region, post: &Post) -> Result<()> {
        let region_str = region.to_string();
        let mut store = self.storage.write().await;

        let id_key = RegionKey {
            region: region_str.clone(),
            id: post.post_id(),
        };
        store.posts_by_id.insert(id_key, post.clone());

        let author_key = RegionKey {
            region: region_str,
            id: post.author_id(),
        };

        let author_posts = store
            .posts_by_author
            .entry(author_key)
            .or_insert_with(Vec::new);

        if let Some(pos) = author_posts
            .iter()
            .position(|p| p.post_id() == post.post_id())
        {
            author_posts[pos] = post.clone();
        } else {
            author_posts.push(post.clone());
        }

        Ok(())
    }

    async fn find_by_id(&self, region: Region, post_id: &PostId) -> Result<Option<Post>> {
        let store = self.storage.read().await;

        let key = RegionKey {
            region: region.to_string(),
            id: *post_id,
        };
        Ok(store.posts_by_id.get(&key).cloned())
    }

    async fn find_by_author(
        &self,
        region: Region,
        author_id: &ProfileId,
        query: PageQuery,
    ) -> Result<PagedResult<Post>> {
        let store = self.storage.read().await;

        let key = RegionKey {
            region: region.to_string(),
            id: *author_id,
        };

        if let Some(author_posts) = store.posts_by_author.get(&key) {
            let limit = query.limit;
            let mut posts = author_posts.clone();

            let total_len = posts.len();
            posts.truncate(limit);

            let next_cursor = if total_len >= limit {
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
        } else {
            Ok(PagedResult {
                items: Vec::new(),
                next_cursor: None,
            })
        }
    }

    async fn delete(&self, region: Region, post_id: &PostId, author_id: &ProfileId) -> Result<()> {
        let region_str = region.to_string();
        let mut store = self.storage.write().await;

        let id_key = RegionKey {
            region: region_str.clone(),
            id: *post_id,
        };
        store.posts_by_id.remove(&id_key);

        let author_key = RegionKey {
            region: region_str,
            id: *author_id,
        };
        if let Some(author_posts) = store.posts_by_author.get_mut(&author_key) {
            author_posts.retain(|p| p.post_id() != *post_id);
        }

        Ok(())
    }
}
