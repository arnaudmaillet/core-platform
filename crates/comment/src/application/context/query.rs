// crates/content_comments/src/application/context/query.rs

use shared_kernel::core::{PageQuery, PagedResult, Result};
use shared_kernel::types::{PostId, ProfileId};
use std::sync::Arc;

use crate::application::dtos::CommentWithProfile;
use crate::repositories::{CommentRepository, CommentUserProfileRepository};
use crate::types::CommentId;

#[async_trait::async_trait]
pub trait AccountResolverClient: Send + Sync {
    async fn fetch_profiles_batch(
        &self,
        profile_ids: &[ProfileId],
    ) -> Result<Vec<crate::types::CommentUserProfile>>;
}

#[derive(Clone)]
pub struct CommentQueryContext {
    comment_repo: Arc<dyn CommentRepository>,
    profile_repo: Arc<dyn CommentUserProfileRepository>,
    account_client: Arc<dyn AccountResolverClient>,
}

impl CommentQueryContext {
    pub fn new(
        comment_repo: Arc<dyn CommentRepository>,
        profile_repo: Arc<dyn CommentUserProfileRepository>,
        account_client: Arc<dyn AccountResolverClient>,
    ) -> Self {
        Self {
            comment_repo,
            profile_repo,
            account_client,
        }
    }

    pub async fn find_roots_by_post(
        &self,
        post_id: PostId,
        query: PageQuery,
    ) -> Result<PagedResult<CommentWithProfile>> {
        let paged_comments = self.comment_repo.find_roots_by_post(post_id, query).await?;

        let items = self
            .hydrate_comments_with_profiles(paged_comments.items)
            .await?;

        Ok(PagedResult {
            items,
            next_cursor: paged_comments.next_cursor,
        })
    }

    pub async fn find_replies_by_parent(
        &self,
        parent_id: CommentId,
        query: PageQuery,
    ) -> Result<PagedResult<CommentWithProfile>> {
        let paged_replies = self
            .comment_repo
            .find_replies_by_parent(parent_id, query)
            .await?;

        let items = self
            .hydrate_comments_with_profiles(paged_replies.items)
            .await?;

        Ok(PagedResult {
            items,
            next_cursor: paged_replies.next_cursor,
        })
    }

    async fn hydrate_comments_with_profiles(
        &self,
        comments: Vec<crate::entities::Comment>,
    ) -> Result<Vec<CommentWithProfile>> {
        if comments.is_empty() {
            return Ok(Vec::new());
        }

        let mut profile_ids: Vec<ProfileId> = comments.iter().map(|c| c.profile_id()).collect();
        profile_ids.sort();
        profile_ids.dedup();

        // Lecture batchée dans la table locale ScyllaDB
        let mut profiles_map = self.profile_repo.find_batch(&profile_ids).await?;

        // Détection des profils manquants (Cold Start / Trous de réplication)
        let missing_ids: Vec<ProfileId> = profile_ids
            .into_iter()
            .filter(|id| !profiles_map.contains_key(id))
            .collect();

        // Fallback synchrone gRPC si nécessaire
        if !missing_ids.is_empty() {
            if let Ok(fetched_profiles) =
                self.account_client.fetch_profiles_batch(&missing_ids).await
            {
                let mut to_save = Vec::new();
                for profile in fetched_profiles {
                    profiles_map.insert(profile.profile_id(), profile.clone());
                    to_save.push(profile);
                }

                // 🚀 Auto-guérison asynchrone : On spawn l'écriture dans ScyllaDB sans bloquer la lecture courante
                let repo_clone = self.profile_repo.clone();
                tokio::spawn(async move {
                    let _ = repo_clone.save_batch(to_save).await;
                });
            }
        }

        // Assemblage final
        let enriched_comments = comments
            .into_iter()
            .map(|comment| {
                let profile = profiles_map.get(&comment.profile_id()).cloned();
                CommentWithProfile { comment, profile }
            })
            .collect();

        Ok(enriched_comments)
    }
}
