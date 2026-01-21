// backend/gateway/graphql-bff/src/context.rs

use std::sync::Arc;
use tokio::sync::RwLock;
use crate::clients::profile::profile_identity_service_client::ProfileIdentityServiceClient;
use crate::clients::profile::profile_query_service_client::ProfileQueryServiceClient;
use tonic::transport::Channel;

pub struct ApiContext {
    // On utilise des clients Tonic qui gèrent déjà le load-balancing interne
    pub profile_identity: ProfileIdentityServiceClient<Channel>,
    pub profile_query: ProfileQueryServiceClient<Channel>,
}

impl ApiContext {
    pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        // Dans un vrai environnement, ces URLs viendraient de tes variables d'env
        let profile_url = "http://[::1]:50051";

        // On crée les channels de connexion
        let profile_channel = Channel::from_static(profile_url)
            .connect()
            .await?;

        Ok(Self {
            profile_identity: ProfileIdentityServiceClient::new(profile_channel.clone()),
            profile_query: ProfileQueryServiceClient::new(profile_channel),
        })
    }
}