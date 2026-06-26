//! The concrete reconciliation source: authoritative follower/following counts
//! from `social-graph` over gRPC.
//!
//! Integration reality (mirrors the decode layer's honesty): only the
//! social-graph-owned profile metrics have a true authoritative count RPC today.
//! `social-graph` is the transactional system-of-record for the follow graph, so
//! its counts are correct and worth reconciling against. Like/Share/Comment
//! reconciliation stays deferred — `engagement` exposes *weighted reaction scores*
//! (not a reaction count), and its share/comment counters are the approximate ones
//! this service supersedes; reconciling against them would be circular.

use async_trait::async_trait;
use social_graph_api::GetRelationStatusRequest;
use social_graph_api::social_graph_service_client::SocialGraphServiceClient;
use tonic::Code;
use tonic::transport::Channel;

use crate::application::port::ReconciliationSource;
use crate::domain::{EntityKind, EntityRef, Metric};
use crate::error::CounterError;

/// Resolves authoritative counts from `social-graph`. The channel is cheap to
/// clone (it is `Arc`-backed), so each call clones the client.
pub struct GrpcReconciliationSource {
    social_graph: SocialGraphServiceClient<Channel>,
}

impl GrpcReconciliationSource {
    pub fn new(social_graph: Channel) -> Self {
        Self {
            social_graph: SocialGraphServiceClient::new(social_graph),
        }
    }
}

#[async_trait]
impl ReconciliationSource for GrpcReconciliationSource {
    async fn authoritative_count(
        &self,
        entity: &EntityRef,
        metric: Metric,
    ) -> Result<Option<i64>, CounterError> {
        // Pick the field; bail for any metric this source does not own.
        let want_followers = match metric {
            Metric::Follower => true,
            Metric::Following => false,
            _ => return Ok(None),
        };
        if entity.kind != EntityKind::Profile {
            return Ok(None);
        }

        // The counts in the view are "for the target profile" regardless of actor;
        // a self-relation query is the simplest way to read them.
        let mut client = self.social_graph.clone();
        let request = GetRelationStatusRequest {
            actor_id: entity.id.as_str().to_owned(),
            target_id: entity.id.as_str().to_owned(),
        };
        match client.get_relation_status(request).await {
            Ok(response) => {
                let view = response.into_inner();
                Ok(Some(if want_followers {
                    view.target_followers_count
                } else {
                    view.target_following_count
                }))
            }
            // Unknown profile / no relations → nothing to reconcile against.
            Err(status) if status.code() == Code::NotFound => Ok(None),
            // Anything else is a transient source fault: the loop logs and moves on.
            Err(status) => Err(CounterError::SourceReplayFailed {
                reason: format!("social-graph get_relation_status: {}", status.message()),
            }),
        }
    }
}
