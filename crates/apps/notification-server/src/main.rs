//! `notification-server` — the deployable notification binary.

use std::net::SocketAddr;

use notification::service::NotificationService;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let addr: SocketAddr = std::env::var("NOTIFICATION_GRPC_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:50055".to_owned())
        .parse()?;

    service_runtime::serve::<NotificationService>(addr).await
}
