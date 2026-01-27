// backend/gateway/graphql-bff/src/context.rs

use std::sync::Arc;
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
        // 1. On récupère l'URL via la variable d'env, avec un fallback pour le dev local
        let profile_url = std::env::var("PROFILE_SERVICE_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:50051".to_string());

        // 2. On utilise Box::leak car tonic/hyper attend souvent une 'static string
        // ou on convertit l'URL en Endpoint
        let endpoint = tonic::transport::Endpoint::from_shared(profile_url)?;

        // 3. On tente la connexion
        let profile_channel = endpoint.connect().await?;

        Ok(Self {
            profile_identity: ProfileIdentityServiceClient::new(profile_channel.clone()),
            profile_query: ProfileQueryServiceClient::new(profile_channel),
        })
    }
}