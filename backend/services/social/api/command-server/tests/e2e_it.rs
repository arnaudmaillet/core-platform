// backend/services/social/api/command-server/tests/social_e2e_it.rs

use auth::Claims;
use auth_test_utils::TokenValidatorStub;
use infra_fred::fred::interfaces::HashesInterface;
use shared_kernel::{
    core::{Identifier, Result},
    types::{ProfileId, SubId},
};
use shared_proto::social::v1::social_service_client::SocialServiceClient;
use shared_proto::social::v1::{CommandTarget, FollowProfileRequest, UnfollowProfileRequest};
use social_test_utils::SocialTestContextBuilder;
use tonic::{Request, metadata::MetadataValue};
use uuid::Uuid;

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

    let test_token = "simulated.social.service.jwt";
    let target_sub_id = "keycloak|social-user-999";
    let mock_validator = std::sync::Arc::new(TokenValidatorStub::new());

    let expected_claims = Claims {
        sub_id: SubId::try_new(target_sub_id)?,
        aud: serde_json::Value::String("social-service".to_string()),
        iss: "https://identity.core.platform/realms/master".to_string(),
        email: None,
        email_verified: None,
        phone_number: None,
        phone_number_verified: None,
        realm_access: None,
        exp: chrono::Utc::now().timestamp() as u64 + 3600,
    };

    mock_validator.stub_token(test_token, expected_claims);

    // 2. SETUP INFRASTRUCTURE E2E
    let ctx = SocialTestContextBuilder::new()
        .with_mock_auth(mock_validator)
        .with_grpc_server()
        .build_e2e()
        .await;

    let mut social_client = SocialServiceClient::connect(ctx.grpc_url()).await.unwrap();

    let follower_id = ProfileId::generate();
    let following_id = ProfileId::generate();

    // 3. ACT : FOLLOW VIA LE CLIENT gRPC
    let follow_req = FollowProfileRequest {
        command_id: Uuid::now_v7().to_string(),
        follower_id: follower_id.to_string(),
        target: Some(CommandTarget {
            profile_id: following_id.to_string(),
            expected_version: 0,
        }),
    };

    let follow_res = social_client
        .follow_profile(with_auth(follow_req, test_token, "EU"))
        .await;
    assert!(
        follow_res.is_ok(),
        "L'appel gRPC de Follow a échoué : {:?}",
        follow_res.err()
    );

    // 4. VERIFICATIONS DE PERSISTANCE (ScyllaDB + Redis)
    let scylla_rows = ctx
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
        scylla_rows.rows_num(),
        1,
        "La relation n'a pas été insérée dans ScyllaDB"
    );

    let redis_pool = ctx.kernel().redis().cache().pool().clone();
    let count_str: Option<String> = redis_pool
        .hget(format!("profile:counters:{}", follower_id), "following")
        .await
        .unwrap();

    let count = count_str.and_then(|s| s.parse::<i64>().ok()).unwrap_or(0);
    assert_eq!(
        count, 1,
        "Le compteur temporaire d'abonnements Redis devrait être à 1"
    );

    // 5. ACT : UNFOLLOW VIA LE CLIENT gRPC
    let unfollow_req = UnfollowProfileRequest {
        command_id: Uuid::now_v7().to_string(),
        follower_id: follower_id.to_string(),
        target: Some(CommandTarget {
            profile_id: following_id.to_string(),
            expected_version: 1,
        }),
    };

    assert!(
        social_client
            .unfollow_profile(with_auth(unfollow_req, test_token, "EU"))
            .await
            .is_ok(),
        "L'appel gRPC d'Unfollow a échoué"
    );

    // 6. VERIFICATIONS DU NETTOYAGE
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
        "ScyllaDB: Le lien relationnel aurait dû être supprimé de la table"
    );

    // Vérifier que le compteur Redis est bien redescendu à 0
    let final_count_str: Option<String> = redis_pool
        .hget(format!("profile:counters:{}", follower_id), "following")
        .await
        .unwrap();

    let final_count = final_count_str
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(0);

    assert_eq!(
        final_count, 0,
        "Redis: Le compteur atomique d'abonnements doit être revenu à 0"
    );

    ctx.shutdown().await;
    Ok(())
}
