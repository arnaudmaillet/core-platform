// backend/services/post/api/command-server/tests/post_e2e_it.rs

use auth_test_utils::KeycloakTestContext;
use infra_fred::fred::interfaces::KeysInterface;
use post_test_utils::PostTestContextBuilder;
use shared_kernel::{
    core::{Identifier, Result},
    types::{PostId, ProfileId, Region},
};
use shared_proto::post::v1::{CreatePostRequest, GetPostRequest};
use shared_proto::post::v1::{QueryMetadata, post_service_client::PostServiceClient};
use tonic::{Request, metadata::MetadataValue};
use uuid::Uuid;

/// Helper d'injection des métadonnées d'authentification gRPC et de routing géographique
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
async fn test_e2e_complete_post_lifecycle_with_cache_aside() -> Result<()> {
    let _ = tracing_subscriber::fmt::try_init();

    let ctx = PostTestContextBuilder::new()
        .with_grpc_server()
        .build_e2e()
        .await;

    let mut post_client = PostServiceClient::connect(ctx.grpc_url()).await.unwrap();
    let auth_ctx = KeycloakTestContext::restore("master").await;
    let auth_response = auth_ctx.get_admin_token().await?;

    let region = Region::default();
    let author_id = ProfileId::generate();

    let create_req = CreatePostRequest {
        command_id: Uuid::now_v7().to_string(),
        region: region.to_string(),
        author_id: author_id.to_string(),
        post_type: "text".to_string(),
        caption: Some(
            "Hyperscale post architecture with custom Redis caching! #rust #scylla".to_string(),
        ),
        visibility_level: "public".to_string(),
        media_list: vec![],
        music_id: None,
        dynamic_metadata: "".to_string(),
        allowed_comment_hands: true,
    };

    tracing::info!("Sending CreatePostRequest through gRPC client...");
    let create_res = post_client
        .create_post(with_auth(create_req, auth_response.token.as_str(), "EU"))
        .await;

    assert!(
        create_res.is_ok(),
        "gRPC creation query failed: {:?}",
        create_res.err()
    );

    let create_payload = create_res.unwrap().into_inner();
    let post_id = PostId::try_from(create_payload.post_id)
        .expect("Le serveur gRPC doit renvoyer un PostId valide");
    tracing::info!(%post_id, "Post successfully created, moving to database verifications.");

    let keyspace = ctx.kernel().scylla().keyspace();
    let scylla_session = ctx.kernel().scylla().session();

    let query_by_id = format!(
        "SELECT post_id, author_id, caption FROM {}.posts_by_id WHERE region = ? AND post_id = ?",
        keyspace
    );
    let query_params: (String, uuid::Uuid) = ("EU".to_string(), post_id.as_uuid());

    let rows_by_id = scylla_session
        .query_unpaged(query_by_id, query_params)
        .await
        .unwrap()
        .into_rows_result()
        .unwrap();

    assert_eq!(
        rows_by_id.rows_num(),
        1,
        "Post must exist in posts_by_id table"
    );

    let query_by_author = format!(
        "SELECT post_id FROM {}.posts_by_author WHERE region = ? AND author_id = ? AND post_id = ?",
        keyspace
    );
    let author_params: (String, uuid::Uuid, uuid::Uuid) =
        ("EU".to_string(), author_id.as_uuid(), post_id.as_uuid());

    let rows_by_author = scylla_session
        .query_unpaged(query_by_author, author_params)
        .await
        .unwrap()
        .into_rows_result()
        .unwrap();

    assert_eq!(
        rows_by_author.rows_num(),
        1,
        "Post must exist in posts_by_author timeline table"
    );

    let redis_pool = ctx.kernel().redis().repository().pool().clone();
    let cache_key = format!("posts:EU:{}", post_id);

    let cache_exists_before: bool = redis_pool.exists(&cache_key).await.unwrap();
    assert!(
        !cache_exists_before,
        "Le cache doit être vide avant la première requête de lecture"
    );

    let get_req = GetPostRequest {
        post_id: post_id.to_string(),
        author_id: author_id.to_string(),
        metadata: Some(QueryMetadata {
            region: "EU".to_string(),
        }),
    };

    let get_res = post_client
        .get_post(with_auth(get_req, auth_response.token.as_str(), "EU"))
        .await;

    assert!(get_res.is_ok(), "gRPC GetPost query failed");
    let returned_post = get_res.unwrap().into_inner();
    assert_eq!(
        returned_post.caption,
        Some(
            "Wynn hyperscale post architecture with custom Redis caching! #rust #scylla"
                .to_string()
        )
    );

    let cache_bytes_after: Option<String> = redis_pool.get(&cache_key).await.unwrap();
    assert!(
        cache_bytes_after.is_some(),
        "Redis cache should be populated following the first GetPost query (Cache Miss Fallback)"
    );

    let cached_json_str = cache_bytes_after.unwrap();
    assert!(
        cached_json_str.contains(&post_id.to_string()),
        "Cached JSON must wrap correct identity context"
    );
    ctx.shutdown().await;
    Ok(())
}
