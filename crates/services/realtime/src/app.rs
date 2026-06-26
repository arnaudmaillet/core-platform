//! The realtime composition roots.
//!
//! [`Adapters::build`] is the I/O variant that constructs the live port adapters
//! from config (the Redis routing fabric + node hop, the auth-context verifier),
//! plus the concrete [`RedisClient`] / [`RedisSubscriber`] the runtime needs for
//! the health probe and the node-channel subscription. Both binaries build from
//! it: the gateway uses every field; the dispatcher uses only the registry + node
//! channel for fan-out.

use std::sync::Arc;

use auth_context::{
    AuthContextConfig, JwksCache, JwksClient, JwksRefresher, JwtDecoder, OidcClaimsExtractor,
};
use jsonwebtoken::Algorithm;
use redis_storage::{RedisClient, RedisClientBuilder, RedisSubscriber, RedisSubscriberBuilder};

use crate::application::port::{ConnectionRegistry, NodeChannel, TokenVerifier};
use crate::config::RealtimeConfig;
use crate::infrastructure::auth_context_verifier::AuthContextTokenVerifier;
use crate::infrastructure::redis_connection_registry::RedisConnectionRegistry;
use crate::infrastructure::redis_node_channel::RedisNodeChannel;

/// The live adapter set the binaries build from.
pub struct Adapters {
    pub registry: Arc<dyn ConnectionRegistry>,
    pub node_channel: Arc<dyn NodeChannel>,
    pub verifier: Arc<dyn TokenVerifier>,
    /// Retained for the runtime's hot-tier liveness probe.
    pub redis: RedisClient,
    /// A dedicated subscriber connection for the gateway's node-channel
    /// (`SSUBSCRIBE`) loop.
    pub subscriber: RedisSubscriber,
}

impl Adapters {
    pub async fn build(config: &RealtimeConfig) -> anyhow::Result<Self> {
        let redis = RedisClientBuilder::new(config.redis.clone()).build().await?;
        let subscriber = RedisSubscriberBuilder::new(config.redis.clone()).build().await?;

        let registry: Arc<dyn ConnectionRegistry> = Arc::new(RedisConnectionRegistry::new(
            redis.clone(),
            config.registry_ttl.as_millis() as i64,
        ));
        let node_channel: Arc<dyn NodeChannel> = Arc::new(RedisNodeChannel::new(redis.clone()));
        let verifier: Arc<dyn TokenVerifier> =
            Arc::new(AuthContextTokenVerifier::new(
                build_decoder(&config.auth),
                config.device_claim.clone(),
            ));

        Ok(Self {
            registry,
            node_channel,
            verifier,
            redis,
            subscriber,
        })
    }
}

/// Build the ES256 edge-token decoder and start its JWKS refresher.
///
/// The refresher is detached (its `JoinHandle` is dropped, which keeps the task
/// running for the process lifetime); the cache it warms is shared with the
/// decoder. A cold start does not require the IdP to be reachable — verification
/// fails closed (`RTM-1001`) until the first successful JWKS fetch.
fn build_decoder(
    auth: &AuthContextConfig,
) -> Arc<JwtDecoder<auth_context::OidcClaims, OidcClaimsExtractor>> {
    let cache = JwksCache::new();
    let client = JwksClient::new(auth.jwks_url.clone(), auth.fetch_timeout);
    let _refresher = JwksRefresher::spawn(
        client,
        cache.clone(),
        auth.refresh_interval,
        auth.max_backoff,
    );
    // The edge token is ES256; accept RS256 too in case the JWKS mixes key types.
    Arc::new(JwtDecoder::with_algorithms(
        auth,
        cache,
        OidcClaimsExtractor::default(),
        vec![Algorithm::ES256, Algorithm::RS256],
    ))
}
