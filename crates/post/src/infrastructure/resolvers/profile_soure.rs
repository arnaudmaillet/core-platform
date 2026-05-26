// crates/post/src/infrastructure/resolvers/grpc_profile_source.rs

use crate::resolvers::ProfileSource;
use async_trait::async_trait;
use shared_kernel::core::{Error, Result};
use shared_kernel::types::ProfileId;
use shared_proto::profile::v1::profile_query_service_client::ProfileQueryServiceClient;
use shared_proto::profile::v1::{QueryMetadata, ResolveSlugsRequest};
use std::collections::{BTreeMap, BTreeSet};
use tonic::transport::Channel;

pub struct GrpcProfileSource {
    client: ProfileQueryServiceClient<Channel>,
    region: String,
}

impl GrpcProfileSource {
    pub fn new(client: ProfileQueryServiceClient<Channel>, region: String) -> Self {
        Self { client, region }
    }
}

#[async_trait]
impl ProfileSource for GrpcProfileSource {
    async fn fetch_from_source(
        &self,
        slugs: &BTreeSet<String>,
    ) -> Result<BTreeMap<String, ProfileId>> {
        let request = tonic::Request::new(ResolveSlugsRequest {
            metadata: Some(QueryMetadata {
                region: self.region.clone(),
            }),
            slugs: slugs.iter().cloned().collect(),
        });

        let mut client = self.client.clone();
        let response = client
            .resolve_slugs(request)
            .await
            .map_err(|e| Error::internal(format!("gRPC Profile service error: {}", e)))?;

        let mappings = response.into_inner().mappings;
        let mut resolved = BTreeMap::new();

        for (slug, id_str) in mappings {
            if let Ok(profile_id) = ProfileId::try_new(&id_str) {
                resolved.insert(slug, profile_id);
            } else {
                tracing::warn!(slug = %slug, "Received invalid ProfileId format from gRPC");
            }
        }

        Ok(resolved)
    }
}
