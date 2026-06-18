// crates/post/core/src/post/domain/stubs/write_stub.rs (ou dans tes dossiers de tests)

use crate::{Post, PostWriteRepository};
use async_trait::async_trait;
use shared_kernel::core::Result;
use shared_kernel::types::{PostId, ProfileId};
use std::collections::HashMap;
use std::sync::RwLock;

pub struct PostWriteRepositoryStub {
    posts: RwLock<HashMap<PostId, Post>>,
}

impl PostWriteRepositoryStub {
    pub fn new() -> Self {
        Self {
            posts: RwLock::new(HashMap::new()),
        }
    }

    pub fn contains(&self, post_id: &PostId) -> bool {
        self.posts.read().unwrap().contains_key(post_id)
    }

    pub fn get_raw(&self, post_id: &PostId) -> Option<Post> {
        self.posts.read().unwrap().get(post_id).cloned()
    }
}

#[async_trait]
impl PostWriteRepository for PostWriteRepositoryStub {
    async fn save(&self, post: &Post) -> Result<()> {
        let mut posts = self.posts.write().unwrap();
        posts.insert(post.post_id().clone(), post.clone());
        Ok(())
    }

    async fn delete(&self, post_id: &PostId, _author_id: &ProfileId) -> Result<()> {
        let mut posts = self.posts.write().unwrap();
        posts.remove(post_id);
        Ok(())
    }
}
