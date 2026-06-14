// crates/post/src/infrastructure/repositories/post_repository_stub.rs

use async_trait::async_trait;
use shared_kernel::core::{Error, PageQuery, PagedResult, Result, Versioned};
use shared_kernel::types::{PostId, ProfileId, Region};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use post::entities::Post;
use post::repositories::PostRepository;

#[derive(Default)]
struct InnerStorage {
    posts_by_id: HashMap<PostId, Post>,
    posts_by_author: HashMap<ProfileId, Vec<Post>>,
    error_to_return: Option<Error>,
}

#[derive(Clone, Default)]
pub struct PostStoreStub {
    storage: Arc<RwLock<InnerStorage>>,
}

impl PostStoreStub {
    pub fn new() -> Self {
        Self {
            storage: Arc::new(RwLock::new(InnerStorage::default())),
        }
    }

    pub async fn save_direct(&self, post: Post) {
        let mut store = self.storage.write().await;
        store.posts_by_id.insert(post.post_id(), post.clone());

        store
            .posts_by_author
            .entry(post.author_id())
            .or_default()
            .push(post);
    }

    pub async fn find_direct(&self, id: PostId) -> Result<Post> {
        let store = self.storage.read().await;
        store
            .posts_by_id
            .get(&id)
            .cloned()
            .ok_or_else(|| Error::not_found("Post", id.to_string()))
    }

    /// Simule une panne d'infrastructure imminente (Scylla Down, timeout network...)
    pub async fn set_error(&self, err: Error) {
        let mut store = self.storage.write().await;
        store.error_to_return = Some(err);
    }

    pub async fn clear(&self) {
        let mut store = self.storage.write().await;
        store.posts_by_id.clear();
        store.posts_by_author.clear();
        store.error_to_return = None;
    }

    pub async fn count_all(&self) -> usize {
        let guard = self.storage.read().await;
        guard.posts_by_id.len()
    }
}

#[async_trait]
impl PostRepository for PostStoreStub {
    async fn save(&self, _region: Region, post: &Post) -> Result<()> {
        let mut store = self.storage.write().await;

        if let Some(err) = store.error_to_return.clone() {
            return Err(err);
        }

        if let Some(existing) = store.posts_by_id.get(&post.post_id()) {
            if existing.version() != post.version() - 1 {
                return Err(Error::concurrency_conflict(format!(
                    "OCC Mismatch dans le Stub : Version actuelle {}, Nouvelle version {}",
                    existing.version(),
                    post.version()
                )));
            }
        }

        store.posts_by_id.insert(post.post_id(), post.clone());

        let author_posts = store.posts_by_author.entry(post.author_id()).or_default();

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

    async fn find_by_id(&self, _region: Region, post_id: &PostId) -> Result<Option<Post>> {
        let store = self.storage.read().await;

        if let Some(err) = &store.error_to_return {
            return Err(err.clone());
        }

        Ok(store.posts_by_id.get(post_id).cloned())
    }

    async fn find_by_author(
        &self,
        _region: Region,
        author_id: &ProfileId,
        query: PageQuery,
    ) -> Result<PagedResult<Post>> {
        let store = self.storage.read().await;

        if let Some(err) = &store.error_to_return {
            return Err(err.clone());
        }

        if let Some(author_posts) = store.posts_by_author.get(author_id) {
            let limit = query.limit;

            let mut posts = author_posts.clone();
            posts.sort_by(|a, b| b.post_id().cmp(&a.post_id()));

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

    async fn delete(&self, _region: Region, post_id: &PostId, author_id: &ProfileId) -> Result<()> {
        let mut store = self.storage.write().await;

        if let Some(err) = &store.error_to_return {
            return Err(err.clone());
        }

        store.posts_by_id.remove(post_id);

        if let Some(author_posts) = store.posts_by_author.get_mut(author_id) {
            author_posts.retain(|p| p.post_id() != *post_id);
        }

        Ok(())
    }
}
