use crate::context::ApiContext;
use crate::schema::build_schema;
use async_graphql_axum::GraphQL;
use axum::Router;
use std::net::SocketAddr;

mod clients;
mod context;
mod domains;
mod schema;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    // 1. Initialisation du contexte (Connexions gRPC)
    let api_context = ApiContext::new().await?;

    // 2. Construction du schéma avec injection du contexte
    // .data() permet de rendre api_context disponible dans tous les resolvers
    let schema = build_schema().await.data(api_context).finish();

    // 3. Setup du serveur
    let app = Router::new().route(
        "/graphql",
        axum::routing::post_service(GraphQL::new(schema.clone())).get_service(GraphQL::new(schema)),
    );

    let addr = SocketAddr::from(([0, 0, 0, 0], 4000));
    println!("🚀 BFF GraphQL Hyperscale sur http://localhost:4000/graphql");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
