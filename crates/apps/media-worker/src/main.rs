//! `media-worker` — the deployable binary for the media **video transcode** path.
//! A one-liner over the shared fleet runtime: the CPU-heavy ffmpeg transcode
//! consumer self-spawns inside `MediaWorkerService::build`. It serves no domain
//! RPC — only gRPC health + reflection (default :50071) so Kubernetes can probe
//! readiness — and is scaled independently of `media-server` on its own node pool.
//! The image control-plane pipeline stays in `media-server`.

use std::net::SocketAddr;

use media::service::MediaWorkerService;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let addr: SocketAddr = std::env::var("MEDIA_WORKER_GRPC_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:50071".to_owned())
        .parse()?;

    service_runtime::serve::<MediaWorkerService>(addr).await
}
