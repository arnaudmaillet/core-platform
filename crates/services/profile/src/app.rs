//! The profile service's composition root.
//!
//! [`App::build`] is *pure composition*: storage configs in, a fully-wired CQRS
//! graph out. It binds no socket and reads no environment, so a binary entrypoint
//! and the live integration harness assemble the exact same graph.
//!
//! Profile is a downstream service — it publishes no events of its own, and its
//! one inbound Kafka touchpoint (the account-event consumer) is wired separately
//! by the serving binary against [`App::command_bus`]; it is intentionally not
//! part of this composition root, so the graph needs no broker.

use std::sync::Arc;

use cqrs::command::{CommandBusBuilder, InMemoryCommandBus};
use cqrs::query::{InMemoryQueryBus, QueryBusBuilder};
use redis_storage::{RedisClientBuilder, RedisConfig};
use scylla_storage::{ScyllaConfig, ScyllaSessionBuilder};

use crate::application::command::{
    ChangeHandleCommand, ChangeHandleHandler, CreateProfileCommand, CreateProfileHandler,
    DeleteProfileCommand, DeleteProfileHandler, HideProfileCommand, HideProfileHandler,
    RestoreProfileCommand, RestoreProfileHandler, SetVisibilityCommand, SetVisibilityHandler,
    UpdateAvatarCommand, UpdateAvatarHandler, UpdateBannerCommand, UpdateBannerHandler,
    UpdateProfileCommand, UpdateProfileHandler, VerifyProfileCommand, VerifyProfileHandler,
};
use crate::application::port::{ProfileCache, ProfileRepository};
use crate::application::query::{
    GetProfileByHandleHandler, GetProfileByHandleQuery, GetProfileByIdHandler, GetProfileByIdQuery,
    ListProfilesByAccountHandler, ListProfilesByAccountQuery,
};
use crate::infrastructure::cache::RedisProfileCache;
use crate::infrastructure::persistence::ScyllaProfileRepository;

/// Storage endpoints the graph is wired against.
pub struct Backends {
    pub scylla: ScyllaConfig,
    pub redis:  RedisConfig,
}

/// A fully-wired profile service bound to its backends, plus the shared `Arc`
/// handles a scenario asserts against. The repository and cache exposed here are
/// the *same* instances the command/query handlers hold.
pub struct App {
    pub command_bus: Arc<InMemoryCommandBus>,
    pub query_bus:   Arc<InMemoryQueryBus>,
    pub repository:  Arc<dyn ProfileRepository>,
    pub cache:       Arc<dyn ProfileCache>,
}

impl App {
    /// Builds storage clients from `backends`, assembles the repository, cache,
    /// and CQRS buses with every profile command and query registered.
    pub async fn build(backends: Backends) -> Result<Self, Box<dyn std::error::Error>> {
        let Backends { scylla, redis } = backends;

        // ── Storage clients ──────────────────────────────────────────────────
        let scylla_client = Arc::new(ScyllaSessionBuilder::new(scylla).build().await?);
        let redis_client = Arc::new(RedisClientBuilder::new(redis).build().await?);

        // ── Adapters (exposed behind their ports) ────────────────────────────
        let repository: Arc<dyn ProfileRepository> =
            Arc::new(ScyllaProfileRepository::new(scylla_client));
        let cache: Arc<dyn ProfileCache> = Arc::new(RedisProfileCache::new(redis_client));

        // ── Command bus ──────────────────────────────────────────────────────
        let command_bus = Arc::new(
            CommandBusBuilder::new()
                .register::<CreateProfileCommand, _>(CreateProfileHandler::new(
                    Arc::clone(&repository),
                    Arc::clone(&cache),
                ))?
                .register::<UpdateProfileCommand, _>(UpdateProfileHandler::new(
                    Arc::clone(&repository),
                    Arc::clone(&cache),
                ))?
                .register::<ChangeHandleCommand, _>(ChangeHandleHandler::new(
                    Arc::clone(&repository),
                    Arc::clone(&cache),
                ))?
                .register::<UpdateAvatarCommand, _>(UpdateAvatarHandler::new(
                    Arc::clone(&repository),
                    Arc::clone(&cache),
                ))?
                .register::<UpdateBannerCommand, _>(UpdateBannerHandler::new(
                    Arc::clone(&repository),
                    Arc::clone(&cache),
                ))?
                .register::<SetVisibilityCommand, _>(SetVisibilityHandler::new(
                    Arc::clone(&repository),
                    Arc::clone(&cache),
                ))?
                .register::<VerifyProfileCommand, _>(VerifyProfileHandler::new(
                    Arc::clone(&repository),
                    Arc::clone(&cache),
                ))?
                .register::<HideProfileCommand, _>(HideProfileHandler::new(
                    Arc::clone(&repository),
                    Arc::clone(&cache),
                ))?
                .register::<RestoreProfileCommand, _>(RestoreProfileHandler::new(
                    Arc::clone(&repository),
                    Arc::clone(&cache),
                ))?
                .register::<DeleteProfileCommand, _>(DeleteProfileHandler::new(
                    Arc::clone(&repository),
                    Arc::clone(&cache),
                ))?
                .build(),
        );

        // ── Query bus ────────────────────────────────────────────────────────
        let query_bus = Arc::new(
            QueryBusBuilder::new()
                .register::<GetProfileByIdQuery, _>(GetProfileByIdHandler::new(
                    Arc::clone(&repository),
                    Arc::clone(&cache),
                ))?
                .register::<GetProfileByHandleQuery, _>(GetProfileByHandleHandler::new(
                    Arc::clone(&repository),
                    Arc::clone(&cache),
                ))?
                .register::<ListProfilesByAccountQuery, _>(ListProfilesByAccountHandler::new(
                    Arc::clone(&repository),
                ))?
                .build(),
        );

        Ok(Self { command_bus, query_bus, repository, cache })
    }
}
