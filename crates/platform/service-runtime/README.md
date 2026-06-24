# service-runtime

The unified fleet bootstrap. Every service runs the **same** boot sequence by
implementing one trait; the deployable binary is then a one-liner.

`serve::<S>(addr)` owns the process-wide concerns so no service re-implements
them:

```text
telemetry::init  (logs + OTLP traces + metrics; guard kept for the process)
  └─ infra-config load  (infrastructure.toml → InfraRegistry, fail-closed at boot)
      └─ spawn_watcher   (hot-reload: resilience / cache / traffic / telemetry)
          └─ [telemetry] sink wired to the live TelemetryControl
              └─ S::build(infra)              (the service's composition root)
                  └─ gRPC server: InboundTraceLayer (outer) + TrafficLayer (inner)
                      ├─ health service  (driven by S::health_probes)
                      └─ S::register(routes)  (the service's own gRPC services)
                          └─ readiness loop + traffic prune loop
                              └─ serve_with_shutdown (SIGINT-drained)
```

## The contract

A service implements `Service`. That is the entire surface:

```rust
use std::sync::Arc;
use service_runtime::{FnProbe, HealthProbe, InfraRegistry, Service};
use tonic::service::RoutesBuilder;

pub struct ChatService { app: App }

#[async_trait::async_trait]
impl Service for ChatService {
    const NAME: &'static str = "chat";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");
    // The health-reporting key: the concrete tonic server's NamedService::NAME.
    const GRPC_SERVICE_NAME: &'static str =
        <ChatServiceServer<ChatServiceHandler<InMemoryCommandBus, InMemoryQueryBus>>
            as tonic::server::NamedService>::NAME;

    /// Pure composition root. `infra` carries the hot-reloadable registries
    /// (resilience / cache / traffic); services that don't consume externalized
    /// policy may ignore it.
    async fn build(infra: Arc<InfraRegistry>) -> anyhow::Result<Self> {
        let app = App::build(/* env-derived config + backends */).await?;
        Ok(Self { app })
    }

    /// Backend liveness probes. The runtime polls these and only reports the
    /// service `SERVING` once all pass — so K8s readiness reflects real
    /// dependency reachability, not mere process liveness. Default: none.
    fn health_probes(&self) -> Vec<Arc<dyn HealthProbe>> {
        let scylla = Arc::clone(&self.app.scylla);
        vec![Arc::new(FnProbe::new("scylla", move || {
            let scylla = Arc::clone(&scylla);
            async move {
                scylla_storage::health::health_check(&scylla.session)
                    .await
                    .map_err(|e| anyhow::anyhow!("scylla: {e}"))
            }
        }))]
    }

    /// Register the concrete gRPC service(s) (typically the service + reflection)
    /// onto the type-erased `routes`. The runtime applies the shared layer stack
    /// and serves, so the layer types never reach this signature.
    fn register(self, routes: &mut RoutesBuilder) -> anyhow::Result<()> {
        let reflection = tonic_reflection::server::Builder::configure()
            .register_encoded_file_descriptor_set(FILE_DESCRIPTOR_SET)
            .build_v1()?;
        routes.add_service(reflection);
        routes.add_service(ChatServiceServer::new(self.app.handler));
        Ok(())
    }
}
```

The deployable binary (`crates/apps/<svc>-server/src/main.rs`) is just:

```rust
use std::net::SocketAddr;
use chat::service::ChatService;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let addr: SocketAddr = std::env::var("CHAT_GRPC_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:50051".to_owned())
        .parse()?;
    service_runtime::serve::<ChatService>(addr).await
}
```

Adding a service to the fleet = a `service.rs` implementing `Service` + an
`apps/<svc>-server` crate this size. Nothing else.

## What the runtime does — and does not — own

| Concern | Owner |
|---|---|
| Telemetry init, OTLP, log/sampling dials | **runtime** (`serve`) |
| Config load + hot-reload watcher | **runtime** |
| Ingress trace + rate-limit layers, prune loop | **runtime** |
| gRPC health, readiness loop | **runtime** |
| Graceful shutdown (SIGINT drain) | **runtime** |
| Domain wiring (repos, caches, buses, workers) | **service** (`build`) |
| Concrete gRPC services + reflection | **service** (`register`) |
| Backend probes | **service** (`health_probes`) |

The split is enforced by the type-erased `RoutesBuilder` seam: `register` never
sees the Tower layer stack the runtime wraps around it.

## Health probes

`HealthProbe` is an async `check()`; [`FnProbe`] adapts a closure so a service
registers a backend check without a bespoke struct. With **no** probes a service
is reported `SERVING` once built; with probes it starts `NOT_SERVING` and flips
on the first successful poll (and back on any failure).

## Environment

| Variable | Default | Effect |
|---|---|---|
| `INFRA_CONFIG_PATH` | `infrastructure.toml` | Externalized-config document path |
| `HEALTH_PROBE_INTERVAL_SECS` | `10` | Readiness poll cadence |
| `TRAFFIC_PRUNE_INTERVAL_SECS` | `60` | Rate-limiter memory-bounding cadence |

Per-service `*_GRPC_ADDR` and tuning variables are documented in each service's
own README. Telemetry honours the standard `RUST_LOG` / `OTEL_*` variables at
boot; the live dials are then driven by the `[telemetry]` section of
`infrastructure.toml`.

## Live retuning

Because the runtime spawns the config watcher, an `infrastructure.toml` push
retunes the fleet with no restart:

```toml
[telemetry]
log_filter = "info,chat=debug"            # live EnvFilter reload
sampling   = { kind = "trace_id_ratio", ratio = 0.05 }   # live, parent-based

[traffic.profiles.standard]
rps = 2000                                  # shadow → enforce, quotas, all live
```
