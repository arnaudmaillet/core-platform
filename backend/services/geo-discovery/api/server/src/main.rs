// // crates/geo_discovery/src/main.rs

// use auth::{KeycloakValidator, interceptors::AuthInterceptor};
// use dotenvy::dotenv;
// use geo_discovery::GeoDiscoveryServiceBuilder;
// use geo_discovery::services::GeoDiscoveryService;
// use infra_fred::{RedisContext, RedisIdempotencyRepository};
// use infra_scylla::ScyllaContext;
// use shared_kernel::types::Region;
// use std::str::FromStr;
// use std::sync::Arc;
// use tonic::transport::Server;
// use tracing_subscriber::{EnvFilter, fmt};

// use shared_proto::geo_discovery::v1::geo_discovery_service_server::GeoDiscoveryServiceServer;

// #[tokio::main]
// async fn main() -> Result<(), Box<dyn std::error::Error>> {
//     // 1. Initialisation du Tracing (Logs de production)
//     let _ = fmt()
//         .with_env_filter(
//             EnvFilter::try_from_default_env()
//                 .unwrap_or_else(|_| EnvFilter::new("info,geo_discovery=debug,tonic=debug")),
//         )
//         .with_test_writer()
//         .try_init();

//     dotenv().ok();
//     tracing::info!("🏁 Starting Geo-Discovery Service Composition Root...");

//     // 2. Récupération des variables d'environnement
//     let scylla_nodes_str =
//         std::env::var("GEO_SCYLLA_NODES").unwrap_or_else(|_| "127.0.0.1:9042".to_string());
//     let redis_url = std::env::var("REDIS_URL").expect("REDIS_URL must be set");
//     let keycloak_url = std::env::var("KEYCLOAK_URL").expect("KEYCLOAK_URL must be set");
//     let keycloak_realm = std::env::var("KEYCLOAK_REALM").expect("KEYCLOAK_REALM must be set");
//     let keycloak_audience =
//         std::env::var("KEYCLOAK_AUDIENCE").unwrap_or_else(|_| "geo-discovery-service".to_string());

//     let port = std::env::var("PORT").unwrap_or_else(|_| "50055".to_string());
//     let addr = format!("0.0.0.0:{}", port).parse()?;

//     // Configuration de la topologie : Détermination de la région physique du Datacenter
//     let region_env = std::env::var("GEO_REGION").unwrap_or_else(|_| "EU".to_string());
//     let current_region = Region::from_str(&region_env).unwrap_or_else(|_| {
//         tracing::warn!("⚠️ Invalid GEO_REGION found, falling back to Region::default()");
//         Region::default()
//     });

//     // 3. Initialisation des clients d'infrastructure (ScyllaDB & Redis)
//     let scylla_nodes: Vec<String> = scylla_nodes_str
//         .split(',')
//         .map(|s| s.trim().to_string())
//         .collect();

//     let scylla_ctx = ScyllaContext::builder_from_env()?
//         .with_nodes(scylla_nodes)
//         .with_keyspace("geo_discovery")
//         .build()
//         .await?;

//     let redis_ctx = RedisContext::builder()?.with_url(redis_url).build().await?;
//     let redis_cache_repo = redis_ctx.repository();
//     let redis_pool = redis_cache_repo.pool().clone();

//     // Dépôt d'idempotence configuré pour le scope géospatial (TTL de 2 heures)
//     let idempotency_repo = Arc::new(RedisIdempotencyRepository::new(
//         redis_pool.clone(),
//         "geo_discovery",
//         7200,
//     ));

//     // Instanciation du résolveur d'engagement (Mock ou implémentation réelle du domaine)
//     let engagement_resolver = Arc::new(
//         geo_discovery::infrastructure::resolvers::engagement_resolver::MockEngagementResolver::new(
//         ),
//     );

//     let max_posts_per_tile = 50;

//     // 4. Assemblage via le Builder Applicatif
//     let builder = GeoDiscoveryServiceBuilder::new(
//         scylla_ctx.session(),
//         redis_pool.clone(),
//         idempotency_repo,
//         engagement_resolver,
//         max_posts_per_tile,
//     );

//     // build_context() instancie automatiquement le canal MPSC et spawn le worker d'hydratation
//     let app_ctx = builder.build_context().await?;

//     // 5. Extraction du contexte de lecture (Query) configuré pour l'infrastructure locale
//     let query_ctx = app_ctx.query(current_region);

//     // 6. Sécurité et Validation de jetons JWT (Keycloak)
//     let validator = Arc::new(
//         KeycloakValidator::new(&keycloak_url, &keycloak_realm, keycloak_audience)
//             .await
//             .expect("Failed to initialize Keycloak validator"),
//     );
//     let auth_interceptor = AuthInterceptor::new(validator.clone());

//     // 7. Instanciation de la couche de présentation gRPC Tonic
//     let geo_discovery_svc = GeoDiscoveryService::new(query_ctx);

//     tracing::info!(
//         "🚀 Geo-Discovery Service active [Region: {:?}] listening on {}",
//         current_region,
//         addr
//     );

//     // 8. Lancement du serveur gRPC avec l'intercepteur de sécurité
//     Server::builder()
//         .add_service(GeoDiscoveryServiceServer::with_interceptor(
//             geo_discovery_svc,
//             auth_interceptor,
//         ))
//         .serve(addr)
//         .await?;

//     Ok(())
// }
fn main() {}
