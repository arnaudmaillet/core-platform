// backend/services/social/api/command-server/tests/social_e2e_it.rs

use auth::{AuthInterceptor, KeycloakTestContext, KeycloakValidator};
use fred::interfaces::{HashesInterface, KeysInterface, SetsInterface};
use shared_kernel::redis::RedisIdempotencyRepository;
use social::test_utils::SocialTestContext;
use std::sync::Arc;
use tonic::transport::Server;
use tonic::{Request, metadata::MetadataValue};
use uuid::Uuid;

// Shared Kernel & Proto Imports
use shared_kernel::{
    core::{Identifier, Result},
    types::{ProfileId, Region, RegionCode},
};
use shared_proto::social::v1::social_service_client::SocialServiceClient;
use shared_proto::social::v1::social_service_server::SocialServiceServer;
use shared_proto::social::v1::{CommandTarget, FollowProfileRequest, UnfollowProfileRequest};

// Social Application Imports
use social::SocialServiceBuilder;
use social::services::SocialService;

// Helper d'injection des métadonnées d'authentification gRPC
fn with_auth<T>(payload: T, token: &str, region: &str) -> Request<T> {
    let mut request = Request::new(payload);
    let token_val = format!("Bearer {}", token)
        .parse::<MetadataValue<_>>()
        .unwrap();
    request.metadata_mut().insert("authorization", token_val);
    request
        .metadata_mut()
        .insert("x-region", region.parse().unwrap());
    request
}

#[tokio::test]
async fn test_e2e_complete_social_graph_lifecycle() -> Result<()> {
    // 1. SETUP INFRA & SERVEUR GRPC VIA LE BUILDER SOCIAL
    let ctx = SocialTestContext::builder()
        .with_server(|deps, addr, shutdown_rx, ready_tx| async move {
            let auth_ctx = KeycloakTestContext::restore("master").await;
            let validator = Arc::new(
                KeycloakValidator::new(&auth_ctx.uri, &auth_ctx.realm)
                    .await
                    .unwrap(),
            );
            let interceptor = AuthInterceptor::new(validator);
            let redis_pool = deps.redis_pool.clone();

            let idempotency_repo = Arc::new(RedisIdempotencyRepository::new(
                redis_pool.clone(),
                "social_e2e",
                300,
            ));

            let builder = SocialServiceBuilder::new(
                deps.scylla,
                redis_pool,
                deps.redis_repo,
                idempotency_repo,
            );

            let app_ctx = builder.build_context().await;
            let bus = builder.build_command_bus();
            let social_svc = SocialService::new(bus, app_ctx);

            ready_tx.send(()).ok();

            Server::builder()
                .add_service(SocialServiceServer::with_interceptor(
                    social_svc,
                    interceptor,
                ))
                .serve_with_shutdown(addr, async {
                    shutdown_rx.await.ok();
                })
                .await
                .unwrap();
        })
        .build_e2e()
        .await;

    let mut social_client = SocialServiceClient::connect(ctx.kernel().grpc_url())
        .await
        .unwrap();
    let auth_ctx = KeycloakTestContext::restore("master").await;
    let auth_response = auth_ctx.get_admin_token().await?;

    let region_str = "EU";
    let region = Region::from_raw(RegionCode::EU);
    let follower_id = ProfileId::generate(region);
    let following_id = ProfileId::generate(region);

    // 2. ACT : FOLLOW (v0 -> v1)
    let command_id = Uuid::now_v7();
    let target = CommandTarget {
        profile_id: following_id.to_string(),
        region: region_str.to_string(),
        expected_version: 0,
    };

    let follow_req = FollowProfileRequest {
        command_id: command_id.to_string(),
        follower_id: follower_id.to_string(),
        target: Some(target),
    };

    let follow_res = social_client
        .follow_profile(with_auth(
            follow_req,
            &auth_response.token.as_str(),
            region_str,
        ))
        .await;

    assert!(follow_res.is_ok(), "Follow gRPC failed");

    // 3. VERIFICATIONS DÉTAILLÉES (Scylla + Redis)
    let scylla_rows = ctx
        .kernel()
        .scylla()
        .session()
        .query_unpaged(
            "SELECT follower_id FROM followings WHERE follower_id = ?",
            (follower_id.as_uuid(),),
        )
        .await
        .unwrap()
        .into_rows_result()
        .unwrap();
    assert_eq!(
        scylla_rows.rows_num(),
        1,
        "ScyllaDB persistency check failed"
    );

    let redis_pool = ctx.kernel().redis().repository().pool().clone();
    let follower_redis_key = format!("profile:counters:{}", follower_id);

    // Check Compteurs Redis
    let count: i64 = redis_pool
        .hget::<i64, _, _>(&follower_redis_key, "following")
        .await
        .unwrap();

    assert_eq!(count, 1, "Redis counter mismatch");

    // Check Dirty Set
    let is_dirty: bool = redis_pool
        .sismember("profiles:dirty", follower_id.to_string())
        .await
        .unwrap();
    assert!(is_dirty, "Dirty marker missing");

    // Check Idempotence
    let idempotency_key = format!("idempotency:social_e2e:{}", command_id);
    let exists: i64 = redis_pool.exists(&idempotency_key).await.unwrap();
    assert!(exists > 0, "Idempotency key missing in Redis");

    // 4. ACT : UNFOLLOW (v1 -> v2)
    let unfollow_req = UnfollowProfileRequest {
        command_id: Uuid::now_v7().to_string(),
        follower_id: follower_id.to_string(),
        target: Some(CommandTarget {
            profile_id: following_id.to_string(),
            region: region_str.to_string(),
            expected_version: 1, // Version attendue suite au follow
        }),
    };

    let unfollow_res = social_client
        .unfollow_profile(with_auth(
            unfollow_req,
            &auth_response.token.as_str(),
            region_str,
        ))
        .await;

    assert!(unfollow_res.is_ok(), "Unfollow gRPC failed");

    ctx.shutdown().await;
    Ok(())
}
