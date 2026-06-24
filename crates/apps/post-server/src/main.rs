//! `post-server` — the deployable post binary.

use std::net::SocketAddr;

use post::service::PostService;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let addr: SocketAddr = std::env::var("POST_GRPC_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:50056".to_owned())
        .parse()?;

    service_runtime::serve::<PostService>(addr).await
}
