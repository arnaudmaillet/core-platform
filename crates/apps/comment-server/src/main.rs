//! `comment-server` — the deployable comment binary.

use std::net::SocketAddr;

use comment::service::CommentService;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let addr: SocketAddr = std::env::var("COMMENT_GRPC_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:50057".to_owned())
        .parse()?;

    service_runtime::serve::<CommentService>(addr).await
}
