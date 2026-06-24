//! `account-server` — the deployable account binary.

use std::net::SocketAddr;

use account::service::AccountService;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let addr: SocketAddr = std::env::var("ACCOUNT_GRPC_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:50059".to_owned())
        .parse()?;

    service_runtime::serve::<AccountService>(addr).await
}
