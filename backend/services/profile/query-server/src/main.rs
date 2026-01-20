use tonic::{transport::Server, Request, Response, Status};

// Import correct suite à la génération Bazel
// backend/services/profile/query-server/src/main.rs
// backend/services/profile/query-server/src/main.rs

// On teste l'accès direct via la crate
// use profile_v1_raw_proto::location::v1::GeoPoint;
// 1. On dit à Rust : "La crate que Bazel appelle 'profile' (les protos),
//    je veux qu'elle s'appelle 'profile_v1' dans ce fichier"

// 2. On fait la même chose pour ta lib métier si nécessaire,
//    ou on laisse Bazel gérer le conflit.
// extern crate profile_logic;

// Le compilateur nous a dit qu'il voyait cette crate :
#[cfg(not(bazel))]
pub mod profile_v1_raw_proto {
    pub mod location {
        pub mod v1 {
            include!("location.v1.rs");
        }
    }
    pub mod profile {
        pub mod v1 {
            include!("profile.v1.rs");
        }
    }
}

use profile_v1_raw_proto::location::v1::GeoPoint;

fn main() {
    let point = GeoPoint::default();
    println!("L'IDE est vert ! Point : {:?}", point);
}

// use profile::application::queries::SearchProfilesUseCase;
// use elasticsearch::Elasticsearch;
//
// pub struct ProfileQueryHandler {
//     search_use_case: SearchProfilesUseCase,
// }
//
// #[tonic::async_trait]
// impl ProfileQueryService for ProfileQueryHandler {
//     async fn autocomplete_profiles(
//         &self,
//         request: Request<AutocompleteRequest>,
//     ) -> Result<Response<AutocompleteResponse>, Status> {
//         let req = request.into_inner();
//
//         // On appelle le Use Case (Logique de recherche ES)
//         let docs = self.search_use_case
//             .execute(&req.query, req.limit)
//             .await
//             .map_err(|e| {
//                 tracing::error!("Search error: {:?}", e);
//                 Status::internal("Search failed")
//             })?;
//
//         // Transformation DTO Infrastructure -> Message gRPC
//         let results = docs.into_iter().map(|d| ProfileSummary {
//             account_id: d.account_id,
//             username: d.username,
//             display_name: d.display_name,
//             avatar_url: d.avatar_url.unwrap_or_default(),
//         }).collect();
//
//         Ok(Response::new(AutocompleteResponse { results }))
//     }
// }
//
// #[tokio::main]
// async fn main() -> anyhow::Result<()> {
//     // Initialisation du traçage pour voir les requêtes gRPC
//     tracing_subscriber::fmt::init();
//
//     // Configuration du client Elasticsearch (Default pointe sur http://localhost:9200)
//     let es_client = Elasticsearch::default();
//
//     // Injection de dépendances
//     let search_use_case = SearchProfilesUseCase::new(es_client);
//     let handler = ProfileQueryHandler { search_use_case };
//
//     let addr = "[::1]:50052".parse()?;
//     tracing::info!("🔍 Profile Query Server listening on {}", addr);
//
//     // Démarrage du serveur
//     Server::builder()
//         .add_service(ProfileQueryServiceServer::new(handler))
//         .serve(addr)
//         .await?;
//
//     Ok(())
// }