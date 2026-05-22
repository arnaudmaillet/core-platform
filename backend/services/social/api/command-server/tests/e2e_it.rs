// backend/services/social/api/command-server/tests/social_e2e_it.rs

use infra_fred::fred::interfaces::HashesInterface;
use infra_test::KeycloakTestContext;
use shared_kernel::{
    core::{Identifier, Result},
    types::{ProfileId, Region, RegionCode},
};
use shared_proto::social::v1::social_service_client::SocialServiceClient;
use shared_proto::social::v1::{CommandTarget, FollowProfileRequest, UnfollowProfileRequest};
use social_test_utils::SocialTestContextBuilder;
use tonic::{Request, metadata::MetadataValue};
use uuid::Uuid;

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
    let _ = tracing_subscriber::fmt::try_init();

    // 1. SETUP HARMONISÉ
    let ctx = SocialTestContextBuilder::new()
        .with_grpc_server()
        .build_e2e()
        .await;

    let mut social_client = SocialServiceClient::connect(ctx.grpc_url()).await.unwrap();
    let auth_ctx = KeycloakTestContext::restore("master").await;
    let auth_response = auth_ctx.get_admin_token().await?;

    let region = Region::from_raw(RegionCode::EU);
    let follower_id = ProfileId::generate(region);
    let following_id = ProfileId::generate(region);

    // 2. ACT : FOLLOW
    let follow_req = FollowProfileRequest {
        command_id: Uuid::now_v7().to_string(),
        follower_id: follower_id.to_string(),
        target: Some(CommandTarget {
            profile_id: following_id.to_string(),
            region: "EU".to_string(),
            expected_version: 0,
        }),
    };

    let follow_res = social_client
        .follow_profile(with_auth(follow_req, &auth_response.token.as_str(), "EU"))
        .await;
    assert!(follow_res.is_ok());

    // 3. VERIFICATIONS (Scylla + Redis)
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
    assert_eq!(scylla_rows.rows_num(), 1);

    let redis_pool = ctx.kernel().redis().repository().pool().clone();
    let count: i64 = redis_pool
        .hget(format!("profile:counters:{}", follower_id), "following")
        .await
        .unwrap();
    assert_eq!(count, 1);

    // 4. ACT : UNFOLLOW
    let unfollow_req = UnfollowProfileRequest {
        command_id: Uuid::now_v7().to_string(),
        follower_id: follower_id.to_string(),
        target: Some(CommandTarget {
            profile_id: following_id.to_string(),
            region: "EU".to_string(),
            expected_version: 1,
        }),
    };

    assert!(
        social_client
            .unfollow_profile(with_auth(unfollow_req, &auth_response.token.as_str(), "EU"))
            .await
            .is_ok()
    );

    // 5. VÉRIFICATION FINALE DE L'ÉTAT (Le "Reverse Cycle")

    // A. Vérifier que la ligne a disparu de Scylla
    let scylla_rows_after = ctx
        .kernel()
        .scylla()
        .session()
        .query_unpaged(
            "SELECT follower_id FROM followings WHERE follower_id = ? AND following_id = ?",
            (follower_id.as_uuid(), following_id.as_uuid()),
        )
        .await
        .unwrap()
        .into_rows_result()
        .unwrap();
    assert_eq!(
        scylla_rows_after.rows_num(),
        0,
        "ScyllaDB: Link should be removed"
    );

    // B. Vérifier que le compteur Redis est revenu à 0
    let redis_pool = ctx.kernel().redis().repository().pool().clone();
    let final_count: Option<i64> = redis_pool
        .hget(format!("profile:counters:{}", follower_id), "following")
        .await
        .unwrap();

    // Soit le champ est supprimé, soit il est à 0
    assert!(final_count.unwrap_or(0) == 0, "Redis: Counter should be 0");

    ctx.shutdown().await;
    Ok(())
}
