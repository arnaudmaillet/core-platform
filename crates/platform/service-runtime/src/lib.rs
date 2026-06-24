//! Unified fleet bootstrap.
//!
//! [`serve`] owns the *one* boot sequence every service in the fleet runs:
//! telemetry init → externalized-config load + hot-reload watcher → service
//! composition → gRPC serving (trace + traffic layers, health, and the service's
//! own routes) → graceful shutdown. A service plugs in by implementing [`Service`];
//! the deployable binary is then a one-liner —
//! `service_runtime::serve::<MyService>(addr).await`.
//!
//! The split of responsibility is deliberate:
//! * the **runtime** owns process-wide concerns (observability, config IO and its
//!   hot-reload watcher, ingress rate-limiting, socket binding, shutdown, and the
//!   **readiness loop** that drives gRPC health from backend liveness) — written
//!   once, here;
//! * the **service** owns only its domain wiring, its concrete tonic services
//!   ([`Service::register`]), and the backend probes it wants polled
//!   ([`Service::health_probes`]).
//!
//! Services register onto a type-erased [`RoutesBuilder`], so the (statically
//! typed) Tower layer stack the runtime wraps every request in — inbound
//! trace-context extraction on the outside, ingress rate-limiting nested within —
//! never leaks into the [`Service`] contract.
//!
//! ## Ingress rate-limiting
//!
//! When the loaded config has a `[traffic]` section, [`serve`] installs
//! transport's `TrafficLayer` (keyed per the bound profile, honouring its
//! hot-reloadable shadow/enforce flag) and spawns a background loop calling
//! [`TrafficRegistry::prune_all`] to bound limiter memory for unbounded
//! keyspaces. With no `[traffic]` section the layer is a transparent pass-through
//! and no prune loop runs.
//!
//! ## Dynamic health
//!
//! The gRPC `grpc.health.v1.Health` status is **not** pinned to `SERVING` at
//! boot. A service reports `SERVING` only once every [`HealthProbe`] it returns
//! has passed at least once, and is demoted to `NOT_SERVING` the moment any probe
//! fails — so Kubernetes readiness reflects real backend reachability.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use async_trait::async_trait;
use infra_config::{
    load_from_path, spawn_watcher, ConfigError, TelemetrySamplingSpec, TelemetrySettings,
    TelemetrySink, TrafficRegistry,
};
// Re-exported so services name the build() parameter type without each taking a
// direct `infra-config` dependency.
pub use infra_config::InfraRegistry;
use telemetry::{SamplingStrategy, TelemetryConfig, TelemetryControl};
use tonic::service::RoutesBuilder;
use tonic_health::server::{health_reporter, HealthReporter};
use tonic_health::ServingStatus;
use transport::grpc::server::{GrpcServerBuilder, GrpcServerConfig};

/// Environment variable naming the externalized-config document.
const INFRA_CONFIG_PATH_ENV: &str = "INFRA_CONFIG_PATH";
/// Path used when [`INFRA_CONFIG_PATH_ENV`] is unset (relative to the working dir).
const DEFAULT_INFRA_CONFIG_PATH: &str = "infrastructure.toml";
/// Environment variable overriding how often [`HealthProbe`]s are polled.
const HEALTH_INTERVAL_ENV: &str = "HEALTH_PROBE_INTERVAL_SECS";
/// Default readiness poll cadence.
const DEFAULT_HEALTH_INTERVAL_SECS: u64 = 10;
/// Environment variable overriding how often the traffic registry is pruned.
const TRAFFIC_PRUNE_INTERVAL_ENV: &str = "TRAFFIC_PRUNE_INTERVAL_SECS";
/// Default traffic-registry prune cadence.
const DEFAULT_TRAFFIC_PRUNE_INTERVAL_SECS: u64 = 60;

/// A liveness probe the runtime polls to drive the service's gRPC health status.
///
/// Implemented by the *service* over its live backend clients (a storage crate
/// can't depend on this platform crate), typically via [`FnProbe`]. Probes must
/// be cheap — they run on every readiness tick.
#[async_trait]
pub trait HealthProbe: Send + Sync + 'static {
    /// Short identifier for logs, e.g. `"scylla"` or `"redis"`.
    fn name(&self) -> &str;

    /// Returns `Ok(())` when the dependency is reachable. Any `Err` demotes the
    /// whole service to `NOT_SERVING` until the next tick clears it.
    async fn check(&self) -> anyhow::Result<()>;
}

/// A [`HealthProbe`] backed by an async closure, so a service registers a backend
/// check without hand-rolling a trait impl per dependency. The closure is `Fn`
/// (re-run every tick), typically capturing a cloned client handle:
///
/// ```ignore
/// FnProbe::new("redis", move || {
///     let client = client.clone();
///     async move {
///         redis_storage::health::health_check(&*client)
///             .await
///             .map_err(|e| anyhow::anyhow!("redis: {e}"))
///     }
/// })
/// ```
pub struct FnProbe<F> {
    name: &'static str,
    check: F,
}

impl<F> FnProbe<F> {
    pub fn new(name: &'static str, check: F) -> Self {
        Self { name, check }
    }
}

#[async_trait]
impl<F, Fut> HealthProbe for FnProbe<F>
where
    F: Fn() -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = anyhow::Result<()>> + Send + 'static,
{
    fn name(&self) -> &str {
        self.name
    }

    async fn check(&self) -> anyhow::Result<()> {
        (self.check)().await
    }
}

/// A fleet service the [`serve`] runtime can host.
///
/// Implementors supply domain wiring ([`build`](Service::build)), registration of
/// their concrete tonic services ([`register`](Service::register)), and —
/// optionally — the backend [`HealthProbe`]s the readiness loop should poll.
/// Everything else (telemetry, config, ingress layers, shutdown, health loop) is
/// the runtime's job.
#[async_trait]
pub trait Service: Sized + Send + 'static {
    /// Stable service name; the telemetry resource and a structured-log field.
    const NAME: &'static str;
    /// Service version, conventionally `env!("CARGO_PKG_VERSION")`.
    const VERSION: &'static str;
    /// Fully-qualified gRPC service name used as the health-reporting key,
    /// i.e. `<ConcreteServer as tonic::server::NamedService>::NAME`.
    const GRPC_SERVICE_NAME: &'static str;

    /// Pure composition root: build the fully-wired service graph.
    ///
    /// `infra` carries the resolved, hot-reloadable infrastructure registries
    /// (resilience / cache / traffic) for services that consume externalized
    /// policy; services that don't may ignore it.
    async fn build(infra: Arc<InfraRegistry>) -> anyhow::Result<Self>;

    /// Backend liveness probes the readiness loop polls. Default: none — the
    /// service is reported `SERVING` as soon as it is built.
    fn health_probes(&self) -> Vec<Arc<dyn HealthProbe>> {
        Vec::new()
    }

    /// Register the service's concrete gRPC service(s) (typically the service plus
    /// reflection) onto the type-erased `routes`. The runtime applies the shared
    /// layer stack and serves, so the layer types never reach this signature.
    fn register(self, routes: &mut RoutesBuilder) -> anyhow::Result<()>;
}

/// Boots and runs a [`Service`] until a shutdown signal, then drains.
///
/// This is the entire production entrypoint: a deployable binary is just
/// `serve::<S>(addr)`.
pub async fn serve<S: Service>(addr: SocketAddr) -> anyhow::Result<()> {
    // ── Observability ──────────────────────────────────────────────────────────
    // The guard must outlive the server: dropping it flushes in-flight spans/logs.
    // Its control handle is wired into the config watcher below for live retuning.
    let telemetry_guard = telemetry::init(TelemetryConfig::from_env(S::NAME, S::VERSION))
        .context("telemetry init")?;

    // ── Externalized config + hot-reload ───────────────────────────────────────
    // Fail-closed at boot: a malformed document stops the pod from ever serving.
    // `_watcher` must stay alive for the process lifetime — dropping it ends the
    // watch and freezes config at its boot value.
    let path = PathBuf::from(
        std::env::var(INFRA_CONFIG_PATH_ENV)
            .unwrap_or_else(|_| DEFAULT_INFRA_CONFIG_PATH.to_owned()),
    );
    let document = load_from_path(&path)
        .with_context(|| format!("load infrastructure config from {}", path.display()))?;
    let infra = Arc::new(
        InfraRegistry::from_config(document).context("resolve infrastructure config")?,
    );

    // Bridge the `[telemetry]` section to the live pipeline: registering the sink
    // applies the boot-time dials immediately, and the watcher pushes every
    // subsequent change — so a config push retunes log filter + sampling with no
    // restart, fleet-wide.
    if let Some(registry) = infra.telemetry() {
        registry
            .set_sink(Arc::new(TelemetryControlSink { control: telemetry_guard.control() }))
            .context("register telemetry control sink")?;
    }

    let _watcher = spawn_watcher(path, Arc::clone(&infra)).context("spawn config watcher")?;

    // ── Compose the service graph ──────────────────────────────────────────────
    let service = S::build(Arc::clone(&infra)).await.context("service build")?;
    let probes = service.health_probes();

    // ── Routes: health (runtime-owned) + the service's own services ────────────
    let (health, health_service) = health_reporter();
    let mut routes = RoutesBuilder::default();
    routes.add_service(health_service);
    service
        .register(&mut routes)
        .context("register grpc routes")?;

    // ── gRPC server: inbound-trace (outer) + traffic (inner) layers ────────────
    let traffic = infra.traffic();
    let mut server_builder = GrpcServerBuilder::new(GrpcServerConfig::default());
    if let Some(registry) = &traffic {
        server_builder = server_builder.with_traffic(Arc::clone(registry));
    }
    let mut server = server_builder.build().context("build gRPC server")?;
    let router = server.add_routes(routes.routes());

    // ── Background loops: readiness health + traffic-memory bounding ────────────
    spawn_readiness(S::GRPC_SERVICE_NAME, health, probes);
    if let Some(registry) = traffic {
        spawn_traffic_prune(registry);
    }

    tracing::info!(service = S::NAME, version = S::VERSION, %addr, "gRPC server listening");

    router
        .serve_with_shutdown(addr, shutdown_signal())
        .await
        .context("grpc server terminated")?;

    tracing::info!(service = S::NAME, "shutdown complete");
    Ok(())
}

/// Spawns the background loop that maps backend [`HealthProbe`] results onto the
/// service's gRPC health status.
///
/// With no probes the service is marked `SERVING` once and the task exits. With
/// probes, the loop polls on [`HEALTH_INTERVAL_ENV`] cadence (first tick fires
/// immediately) and only writes the reporter on a *transition*, so watchers
/// aren't churned every tick.
fn spawn_readiness(
    service_name: &'static str,
    health: HealthReporter,
    probes: Vec<Arc<dyn HealthProbe>>,
) {
    if probes.is_empty() {
        tokio::spawn(async move {
            health
                .set_service_status(service_name, ServingStatus::Serving)
                .await;
        });
        return;
    }

    let interval = interval_from_env(HEALTH_INTERVAL_ENV, DEFAULT_HEALTH_INTERVAL_SECS);

    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        // `None` until the first poll establishes a baseline, so the first result
        // is always written.
        let mut last_serving: Option<bool> = None;

        loop {
            ticker.tick().await;

            let mut serving = true;
            for probe in &probes {
                if let Err(error) = probe.check().await {
                    serving = false;
                    tracing::warn!(
                        service = service_name,
                        probe = probe.name(),
                        %error,
                        "health probe failed; marking NOT_SERVING"
                    );
                    break; // one failed dependency is enough to fail readiness
                }
            }

            if last_serving != Some(serving) {
                let status = if serving {
                    ServingStatus::Serving
                } else {
                    ServingStatus::NotServing
                };
                health.set_service_status(service_name, status).await;
                tracing::info!(service = service_name, serving, "gRPC health status changed");
                last_serving = Some(serving);
            }
        }
    });
}

/// Spawns the background loop that bounds rate-limiter memory by dropping idle
/// keys across every traffic profile on [`TRAFFIC_PRUNE_INTERVAL_ENV`] cadence.
/// Cheap for bounded (`per_method`) profiles; essential for unbounded
/// (`per_caller`) ones.
fn spawn_traffic_prune(registry: Arc<TrafficRegistry>) {
    let interval = interval_from_env(
        TRAFFIC_PRUNE_INTERVAL_ENV,
        DEFAULT_TRAFFIC_PRUNE_INTERVAL_SECS,
    );

    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        ticker.tick().await; // first tick is immediate; nothing to prune at t=0
        loop {
            ticker.tick().await;
            registry.prune_all();
            tracing::debug!(tracked_keys = registry.tracked_keys(), "traffic registry pruned");
        }
    });
}

/// Reads a seconds-valued interval from `env_var`, falling back to `default_secs`
/// when unset or unparseable.
fn interval_from_env(env_var: &str, default_secs: u64) -> Duration {
    Duration::from_secs(
        std::env::var(env_var)
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(default_secs),
    )
}

/// Resolves when the process receives SIGINT (Ctrl-C). The tonic server drains
/// in-flight requests before returning. If the handler cannot be installed we
/// park forever rather than shutting down spuriously.
async fn shutdown_signal() {
    if tokio::signal::ctrl_c().await.is_err() {
        tracing::error!("failed to install Ctrl-C handler; shutdown signalling disabled");
        std::future::pending::<()>().await;
    }
    tracing::info!("shutdown signal received; draining in-flight requests");
}

/// Bridges the `infra-config` `[telemetry]` section to the live telemetry
/// pipeline. Lives here (the platform tier) because it depends on both
/// `infra-config` and `telemetry`, which must not depend on each other.
struct TelemetryControlSink {
    control: TelemetryControl,
}

impl TelemetrySink for TelemetryControlSink {
    fn apply(&self, settings: &TelemetrySettings) -> Result<(), ConfigError> {
        if let Some(directive) = &settings.log_filter {
            self.control
                .set_log_filter(directive)
                .map_err(|e| ConfigError::validation(format!("telemetry log filter: {e}")))?;
        }
        if let Some(spec) = &settings.sampling {
            let strategy = match spec {
                TelemetrySamplingSpec::AlwaysOn => SamplingStrategy::AlwaysOn,
                TelemetrySamplingSpec::AlwaysOff => SamplingStrategy::AlwaysOff,
                TelemetrySamplingSpec::TraceIdRatio { ratio } => {
                    SamplingStrategy::TraceIdRatio(*ratio)
                }
            };
            self.control
                .set_sampling(strategy)
                .map_err(|e| ConfigError::validation(format!("telemetry sampling: {e}")))?;
        }
        Ok(())
    }
}
