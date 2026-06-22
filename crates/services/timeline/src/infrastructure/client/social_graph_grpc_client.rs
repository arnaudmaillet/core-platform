use async_trait::async_trait;
use tonic::transport::Channel;

use crate::application::port::SocialGraphClient;
use crate::domain::value_object::{AuthorId, ProfileId};
use crate::error::TimelineError;

// ── Generated social-graph client stubs ──────────────────────────────────────

mod sg_proto {
    tonic::include_proto!("social_graph.v1");
}

use sg_proto::{
    social_graph_service_client::SocialGraphServiceClient,
    ListFollowersRequest, ListFollowingRequest,
};

// ── Client ────────────────────────────────────────────────────────────────────

/// tonic gRPC client adapter for services/social-graph.
///
/// Both `list_all_followers` and `list_all_following` paginate internally
/// until `next_page_token` is empty, returning the complete flattened list.
/// This is safe because:
///   - Fan-out (followers): called from a background Kafka worker — no latency SLA.
///   - Following list: called at most once per cold-start rebuild — amortized over
///     the `TIMELINE_WARM_TTL_SECS` (default 24h) window.
pub struct SocialGraphGrpcClient {
    channel: Channel,
}

impl SocialGraphGrpcClient {
    pub fn new(channel: Channel) -> Self {
        Self { channel }
    }

    fn client(&self) -> SocialGraphServiceClient<Channel> {
        SocialGraphServiceClient::new(self.channel.clone())
    }
}

#[async_trait]
impl SocialGraphClient for SocialGraphGrpcClient {
    async fn list_all_followers(
        &self,
        author_id: &AuthorId,
        page_size: i32,
    ) -> Result<Vec<ProfileId>, TimelineError> {
        let mut client      = self.client();
        let mut all_ids     = Vec::new();
        let mut page_token  = String::new();
        let followee_id_str = author_id.to_string();

        loop {
            let resp = client
                .list_followers(ListFollowersRequest {
                    followee_id: followee_id_str.clone(),
                    limit:       page_size,
                    page_token:  page_token.clone(),
                })
                .await
                .map_err(|e| TimelineError::SocialGraphClientError {
                    message: e.to_string(),
                })?
                .into_inner();

            for edge in &resp.followers {
                let pid = ProfileId::try_from(edge.profile_id.as_str())
                    .map_err(|_| TimelineError::SocialGraphInvalidId(edge.profile_id.clone()))?;
                all_ids.push(pid);
            }

            if resp.next_page_token.is_empty() {
                break;
            }
            page_token = resp.next_page_token;
        }

        Ok(all_ids)
    }

    async fn list_all_following(
        &self,
        profile_id: &ProfileId,
        page_size:  i32,
    ) -> Result<Vec<AuthorId>, TimelineError> {
        let mut client       = self.client();
        let mut all_ids      = Vec::new();
        let mut page_token   = String::new();
        let follower_id_str  = profile_id.to_string();

        loop {
            let resp = client
                .list_following(ListFollowingRequest {
                    follower_id: follower_id_str.clone(),
                    limit:       page_size,
                    page_token:  page_token.clone(),
                })
                .await
                .map_err(|e| TimelineError::SocialGraphClientError {
                    message: e.to_string(),
                })?
                .into_inner();

            for edge in &resp.following {
                let aid = AuthorId::try_from(edge.profile_id.as_str())
                    .map_err(|_| TimelineError::SocialGraphInvalidId(edge.profile_id.clone()))?;
                all_ids.push(aid);
            }

            if resp.next_page_token.is_empty() {
                break;
            }
            page_token = resp.next_page_token;
        }

        Ok(all_ids)
    }
}
