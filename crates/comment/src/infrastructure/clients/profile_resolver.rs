// crates/content_comments/src/infrastructure/clients/profile_resolver.rs

use async_trait::async_trait;
use tonic::transport::Channel;

use shared_kernel::core::{Error, Result};
use shared_kernel::types::ProfileId;

use shared_proto::profile::v1::GetProfilesBatchRequest;
use shared_proto::profile::v1::profile_query_service_client::ProfileQueryServiceClient;

use crate::types::CommentUserProfile;

pub struct CommentProfileResolver {
    client: ProfileQueryServiceClient<Channel>,
}

impl CommentProfileResolver {
    pub fn new(channel: Channel) -> Self {
        Self {
            client: ProfileQueryServiceClient::new(channel),
        }
    }
}

#[async_trait]
impl ProfileQueryServiceClient for CommentProfileResolver {
    async fn fetch_profiles_batch(
        &self,
        profile_ids: &[ProfileId],
    ) -> Result<Vec<CommentUserProfile>> {
        if profile_ids.is_empty() {
            return Ok(Vec::new());
        }

        // 1. Préparation de la requête Protobuf gRPC
        let proto_ids = profile_ids.iter().map(|id| id.to_string()).collect();
        let request = tonic::Request::new(GetProfilesBatchRequest {
            profile_ids: proto_ids,
        });

        let mut client = self.client.clone();
        let response = client
            .get_profiles_batch(request)
            .await
            .map_err(|e| Error::internal(format!("Account gRPC resolution failed: {}", e)))?;

        let proto_profiles = response.into_inner().profiles;

        let mut domain_profiles = Vec::with_capacity(proto_profiles.len());
        for proto_p in proto_profiles {
            match CommentUserProfile::try_from(proto_p) {
                Ok(profile) => domain_profiles.push(profile),
                Err(e) => {
                    tracing::error!(error = %e, "Failed to map proto profile to CommentUserProfile");
                }
            }
        }

        Ok(domain_profiles)
    }
}
