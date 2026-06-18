// crates/post/assembly/src/read_service.rs

use crate::PostDetail;
use post::PostReadRepository;
use post_profile::ProfileReadProjection;
use shared_kernel::core::{Error, PageQuery, PagedResult, Result};
use shared_kernel::types::{PostId, ProfileId};
use std::sync::Arc;

pub struct PostAssemblyQueryService {
    pub post_reader: Arc<dyn PostReadRepository>,
    pub profile_reader: Arc<dyn ProfileReadProjection>,
}

impl PostAssemblyQueryService {
    pub async fn get_post_detail(&self, id: &PostId) -> Result<Option<PostDetail>> {
        let post = match self.post_reader.find_by_id(id).await? {
            Some(p) => p,
            None => return Ok(None),
        };
        let author_option = self.profile_reader.find_by_id(&post.author_id()).await?;

        let author = author_option.ok_or_else(|| {
            Error::not_found(
                "Profile",
                format!(
                    "Author {} for post {} vanished from projection",
                    post.author_id(),
                    id
                ),
            )
        })?;

        Ok(Some(PostDetail { post, author }))
    }

    pub async fn get_posts_by_author_details(
        &self,
        author_id: &ProfileId,
        query: PageQuery,
    ) -> Result<PagedResult<PostDetail>> {
        let author_option = self.profile_reader.find_by_id(author_id).await?;
        let author = author_option.ok_or_else(|| {
            Error::not_found(
                "Profile",
                format!("Author {} vanished from projection", author_id),
            )
        })?;

        let paged_posts = self.post_reader.find_by_author(author_id, query).await?;
        let items = paged_posts
            .items
            .into_iter()
            .map(|post| PostDetail {
                post,
                author: author.clone(),
            })
            .collect();

        Ok(PagedResult {
            items,
            next_cursor: paged_posts.next_cursor,
        })
    }
}
