mod utils;
use auth::Claims;
use auth_test_utils::TokenValidatorStub;
use post_proto_bridge::v1::CreatePostRequest;
use post_proto_bridge::v1::post_command_service_client::PostCommandServiceClient;
use shared_kernel::{
    core::{Identifier, Result},
    types::{PostId, ProfileId, Region, SubId},
};
use tonic::{Request, metadata::MetadataValue};
use utils::PostTestContextBuilder;
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

    let test_token = "simulated.post.service.jwt";
    let target_sub_id = "keycloak|author-123456";

    // 1. SETUP DU VALIDATEUR MOCKÉ
    let mock_validator = std::sync::Arc::new(TokenValidatorStub::new());

    let expected_claims = Claims {
        sub_id: SubId::try_new(target_sub_id)?,
        aud: serde_json::Value::String("post-service".to_string()),
        iss: "https://identity.core.platform/realms/master".to_string(),
        email: None,
        email_verified: None,
        phone_number: None,
        phone_number_verified: None,
        realm_access: None,
        exp: chrono::Utc::now().timestamp() as u64 + 3600,
    };

    mock_validator.stub_token(test_token, expected_claims);

    // 2. SETUP INFRASTRUCTURE (Initialise le serveur de commande)
    let ctx = PostTestContextBuilder::new()
        .with_mock_auth(mock_validator)
        .with_grpc_server()
        .build_e2e()
        .await;

    // Connexion via le client de commande exclusif
    let mut post_command_client = PostCommandServiceClient::connect(ctx.grpc_url())
        .await
        .unwrap();

    let region = Region::default();
    let author_id = ProfileId::generate();
    let target_command_id = Uuid::new_v4().to_string();

    let create_req = CreatePostRequest {
        command_id: target_command_id.clone(),
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

    // ACT : ENVOI DE LA COMMANDE DE CRÉATION DE POST
    let create_res = post_command_client
        .create_post(with_auth(create_req, test_token, "EU"))
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

    // =========================================================================
    // VERIFICATIONS NO SQL (ScyllaDB) - Validation de la persistance régionale
    // =========================================================================
    let keyspace = ctx.kernel().scylla().keyspace();
    let scylla_session = ctx.kernel().scylla().session();

    let query_by_id = format!(
        "SELECT post_id, author_id, caption FROM {}.posts_by_id WHERE post_id = ?",
        keyspace
    );
    let query_params: (uuid::Uuid,) = (post_id.as_uuid(),);

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

    let mut rows = rows_by_id.rows().unwrap();
    let next_row_result = rows.next().unwrap();
    let (_db_post_id, _db_author_id, db_caption): (uuid::Uuid, uuid::Uuid, Option<String>) =
        next_row_result.unwrap();

    assert_eq!(
        db_caption,
        Some("Hyperscale post architecture with custom Redis caching! #rust #scylla".to_string())
    );

    let query_by_author = format!(
        "SELECT post_id FROM {}.posts_by_author WHERE author_id = ? AND post_id = ?",
        keyspace
    );
    let author_params: (uuid::Uuid, uuid::Uuid) = (author_id.as_uuid(), post_id.as_uuid());

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

    ctx.shutdown().await;
    Ok(())
}
