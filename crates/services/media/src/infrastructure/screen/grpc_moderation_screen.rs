use async_trait::async_trait;
use moderation_api::moderation_service_client::ModerationServiceClient;
use moderation_api::{
    ContentHash as ModContentHash, EntityType, PolicyCategory, ScreenRequest, ScreenVerdict,
    SubjectRef,
};
use tonic::transport::Channel;

use crate::application::port::{ModerationScreen, ScreenDecision};
use crate::domain::value_object::{AssetId, ContentHash, MediaKind};
use crate::error::MediaError;

/// gRPC implementation of [`ModerationScreen`], backed by the `moderation` service's
/// Plane C Screen RPC. The tonic client is cheaply cloneable (the `Channel` is
/// `Arc`-backed), so each call clones it to satisfy the `&self` port signature.
///
/// **Fail-closed:** any transport/gRPC error maps to
/// [`MediaError::ScreenUnavailable`] — the caller converts that into a hard block
/// for CSAM-class media, never an optimistic publish.
#[derive(Clone)]
pub struct GrpcModerationScreen {
    client: ModerationServiceClient<Channel>,
}

impl GrpcModerationScreen {
    pub fn new(channel: Channel) -> Self {
        Self { client: ModerationServiceClient::new(channel) }
    }
}

#[async_trait]
impl ModerationScreen for GrpcModerationScreen {
    async fn screen(
        &self,
        asset_id: &AssetId,
        content_hash: &ContentHash,
        _kind: MediaKind,
    ) -> Result<ScreenDecision, MediaError> {
        let mut client = self.client.clone();
        let request = ScreenRequest {
            subject: Some(SubjectRef {
                entity_type: EntityType::Media as i32,
                entity_id: asset_id.as_str(),
                // A hash match is about content, not the actor; left empty here.
                actor_id: String::new(),
                surface: "upload".to_owned(),
            }),
            hashes: vec![ModContentHash {
                algorithm: "sha256".to_owned(),
                value: content_hash.as_str().to_owned(),
            }],
            // No transient text on the media path; empty categories ⇒ all
            // zero-tolerance categories are screened.
            text: String::new(),
            categories: Vec::new(),
        };

        match client.screen(request).await {
            Ok(resp) => {
                let r = resp.into_inner();
                let verdict =
                    ScreenVerdict::try_from(r.verdict).unwrap_or(ScreenVerdict::Unspecified);
                // BLOCK and the ambiguous REVIEW both hold publication (fail-closed).
                let blocked = matches!(verdict, ScreenVerdict::Block | ScreenVerdict::Review);
                let csam = r.matched_categories.contains(&(PolicyCategory::Csam as i32));
                let reference = (!r.match_reference.is_empty()).then_some(r.match_reference);
                Ok(ScreenDecision { blocked, csam, reference })
            }
            // Fail-closed: an unavailable gate is treated as a block upstream.
            Err(_status) => Err(MediaError::ScreenUnavailable),
        }
    }
}
