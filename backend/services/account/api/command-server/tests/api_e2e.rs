// use account::grpc::GrpcPersonalService;
// use shared_kernel::errors::Result;
// use shared_kernel::infrastructure::utils::E2EServerStarter;
// use shared_proto::account::v1::account_personal_service_client::AccountPersonalServiceClient;
// use shared_proto::account::v1::account_personal_service_server::AccountPersonalServiceServer;
// use sqlx::types::Uuid;
// use std::sync::Arc;
// use std::{net::SocketAddr, path::Path};
// use tonic::transport::Server;
// use tonic::{Request, metadata::MetadataValue};

// // Imports de ton architecture
// use auth::{KeycloakTestContext, KeycloakValidator, TokenValidator};
// use shared_kernel::domain::repositories::CacheRepository;
// use shared_proto::account::v1::{
//     RegisterRequest, RegistrationIdentifier, UpdateLocaleRequest,
//     account_access_service_client::AccountAccessServiceClient,
//     account_access_service_server::AccountAccessServiceServer, registration_identifier::Method,
// };

// use account::{AccountServiceBuilder, grpc::GrpcAccessService};

// struct AccountServerStarter;

// #[async_trait::async_trait]
// impl E2EServerStarter for AccountServerStarter {
//     async fn start_server(
//         &self,
//         pg_pool: sqlx::PgPool,
//         redis_repo: Arc<dyn CacheRepository>,
//         addr: SocketAddr,
//         shutdown_rx: tokio::sync::oneshot::Receiver<()>,
//     ) {
//         let auth_ctx = KeycloakTestContext::restore("master").await;
//         let validator = Arc::new(
//             KeycloakValidator::new(&auth_ctx.uri, &auth_ctx.realm)
//                 .await
//                 .unwrap(),
//         );
//         let interceptor = auth::AuthInterceptor::new(validator);

//         let builder = AccountServiceBuilder::new(pg_pool, redis_repo);
//         let app_ctx = builder.build_context();
//         let bus = builder.build_command_bus();

//         // 3. Instanciation des services gRPC avec les mêmes composants
//         let access_svc = GrpcAccessService::new(bus.clone(), app_ctx.clone());
//         let personal_svc = GrpcPersonalService::new(bus, app_ctx);

//         // 4. Lancement du serveur gRPC unique
//         Server::builder()
//             .add_service(AccountAccessServiceServer::with_interceptor(
//                 access_svc,
//                 interceptor.clone(),
//             ))
//             .add_service(AccountPersonalServiceServer::with_interceptor(
//                 personal_svc,
//                 interceptor,
//             ))
//             .serve_with_shutdown(addr, async {
//                 shutdown_rx.await.ok();
//             })
//             .await
//             .unwrap();
//     }
// }

// // --- HELPER POUR LE TOKEN ---
// fn with_auth<T>(payload: T, token: &str, region: &str) -> Request<T> {
//     let mut request = Request::new(payload);
//     let token_val = format!("Bearer {}", token)
//         .parse::<MetadataValue<_>>()
//         .unwrap();
//     request.metadata_mut().insert("authorization", token_val);
//     request
//         .metadata_mut()
//         .insert("x-region", region.parse().unwrap());
//     request
// }

// #[tokio::test]
// async fn test_e2e_complete_account_lifecycle() -> Result<()> {
//     // 1. SETUP INFRA
//     let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
//     let kernel_migs = manifest_dir.join("../../../../../crates/shared-kernel/migrations/postgres");
//     let account_migs = manifest_dir.join("../../../../../crates/account/migrations/postgres");

//     let ctx = shared_kernel::infrastructure::utils::InfrastructureKernelTestContext::builder()
//         .with_postgres(&[
//             kernel_migs.to_str().unwrap(),
//             account_migs.to_str().unwrap(),
//         ])
//         .with_redis()
//         .with_server(AccountServerStarter)
//         .build_e2e()
//         .await;

//     let mut access_client = AccountAccessServiceClient::connect(ctx.grpc_url())
//         .await
//         .unwrap();
//     let mut personal_client = AccountPersonalServiceClient::connect(ctx.grpc_url())
//         .await
//         .unwrap();

//     let auth_ctx = KeycloakTestContext::restore("master").await;
//     let auth_response = auth_ctx.get_admin_token().await?;

//     // --- UTILISE TON VALIDATEUR POUR EXTRAIRE LE VRAI SUB ---
//     let claims = auth_ctx
//         .validator
//         .validate(&auth_response.token)
//         .expect("Le token généré par Keycloak doit être validable par notre propre validator");

//     let true_admin_id = claims.sub_id.to_string();

//     println!("DEBUG TEST: Le VRAI sub_id du token est: {}", true_admin_id);

//     let email = "audit-e2e@test.com";
//     let region_code = "EU";
//     let command_id = Uuid::now_v7().to_string();

//     // --- ÉTAPE 1 : REGISTER (SUCCESS) ---
//     let register_payload = RegisterRequest {
//         command_id: command_id.clone(),
//         sub_id: Some(true_admin_id.clone()),
//         identifier: Some(RegistrationIdentifier {
//             method: Some(Method::Email(email.to_string())),
//         }),
//         region_code: region_code.to_string(),
//         locale: "fr-FR".to_string(),
//         ip_addr: "127.0.0.1".to_string(),
//     };

//     let res = access_client
//         .register(with_auth(
//             register_payload.clone(),
//             auth_response.token.as_str(),
//             &region_code,
//         ))
//         .await;

//     if let Err(ref e) = res {
//         panic!(
//             "❌ First register failed with status: {:?}. Message: {}",
//             e.code(),
//             e.message()
//         );
//     }

//     assert!(res.is_ok(), "First register failed");

//     // ON RÉCUPÈRE L'ID RÉEL GÉNÉRÉ PAR LE SERVEUR
//     let created_account_identity = res.unwrap().into_inner();
//     let real_account_id = created_account_identity.account_id;
//     assert!(
//         real_account_id.contains(&true_admin_id),
//         "L'ID retourné doit contenir l'UUID original mais peut être préfixé par la région"
//     );
//     assert!(
//         real_account_id.starts_with(region_code),
//         "L'ID doit commencer par le code région"
//     );

//     // --- ÉTAPE 2 : REGISTER (DUPLICATED / IDEMPOTENCY) ---
//     // On renvoie exactement le même payload (même command_id, même sub_id None)
//     let res_dup = access_client
//         .register(with_auth(
//             register_payload,
//             auth_response.token.as_str(),
//             &region_code,
//         ))
//         .await;

//     assert!(res_dup.is_ok(), "Idempotency should return OK");

//     // --- ÉTAPE 3 : UPDATE (CHANGE LOCALE) ---
//     let new_locale = "en-US";
//     let update_payload = UpdateLocaleRequest {
//         command_id: sqlx::types::Uuid::now_v7().to_string(),
//         account_id: real_account_id.clone(), // On utilise l'ID récupéré à l'étape 1
//         locale: new_locale.to_string(),
//     };

//     let upd_res = personal_client
//         .update_locale(with_auth(
//             update_payload,
//             auth_response.token.as_str(),
//             &region_code,
//         ))
//         .await;

//     assert!(upd_res.is_ok(), "Update failed: {:?}", upd_res.err());

//     // --- ÉTAPE 4 : VÉRIFICATIONS FINALES ---

//     let uuid_part = real_account_id.split(':').last().unwrap();
//     let parsed_uuid = sqlx::types::Uuid::parse_str(uuid_part).unwrap();

//     // 1. Vérification Locale & Version
//     let row: (String, i64) = sqlx::query_as(
//         "SELECT locale, version FROM account_identity WHERE account_id = $1 AND region_code = $2",
//     )
//     .bind(parsed_uuid)
//     .bind(region_code) // Sharding key
//     .fetch_one(&ctx.postgres().pool())
//     .await
//     .expect("Account should exist in DB");

//     assert_eq!(row.0, new_locale);
//     // On attend 2 car :
//     // v0 = Init record in bdd
//     // v1 = Register
//     // v2 = Update
//     assert_eq!(
//         row.1, 2,
//         "La version devrait être 2 (1: creation + 1: update)"
//     );

//     // 2. Vérification Idempotence
//     let count: (i64,) =
//         sqlx::query_as("SELECT COUNT(*) FROM processed_commands WHERE command_id = $1")
//             .bind(sqlx::types::Uuid::parse_str(&command_id).unwrap())
//             .fetch_one(&ctx.postgres().pool())
//             .await
//             .unwrap();

//     assert_eq!(
//         count.0, 1,
//         "Should only have ONE record for the same command_id"
//     );

//     ctx.shutdown().await;

//     Ok(())
// }
