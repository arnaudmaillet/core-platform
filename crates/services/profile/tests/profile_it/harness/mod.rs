//! Integration harness: boots the shared infra, wires a real profile graph
//! against it through the production composition root, and exposes the buses plus
//! the repository and Redis cache handles for assertions.
#![allow(dead_code)]

use std::sync::Arc;
use std::time::Duration;

use uuid::Uuid;

use cqrs::command::InMemoryCommandBus;
use cqrs::query::InMemoryQueryBus;
use cqrs::{CommandBus, CqrsError, Envelope, QueryBus};
use redis_storage::RedisConfig;
use scylla_storage::ScyllaConfig;

use profile::app::{App, Backends};
use profile::application::command::{CreateProfileCommand, UpdateProfileCommand};
use profile::application::port::{ProfileCache, ProfileRepository};
use profile::application::query::{GetProfileByHandleQuery, GetProfileByIdQuery};

pub use profile::application::port::profile_cache::ProfileView;
pub use profile::domain::value_object::ProfileId;
pub use test_support::await_until;

/// Generous default patience for a cross-component assertion (ScyllaDB LWT,
/// Redis read-through warm / invalidation round-trip).
pub const DEADLINE: Duration = Duration::from_secs(10);

/// ScyllaDB keyspace the migrations provision.
const KEYSPACE: &str = "profile";
/// On-disk migration assets, resolved against *this* crate's manifest.
const MIGRATIONS_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/migrations");

/// A fully-wired profile service bound to ephemeral infra, plus assertion handles.
pub struct TestHarness {
    pub command_bus: Arc<InMemoryCommandBus>,
    pub query_bus:   Arc<InMemoryQueryBus>,
    pub repository:  Arc<dyn ProfileRepository>,
    pub cache:       Arc<dyn ProfileCache>,
}

impl TestHarness {
    /// Boots/reuses the shared containers, applies migrations, and assembles the
    /// service graph.
    pub async fn start() -> Self {
        let scylla_cp = test_support::containers::scylla_ready(KEYSPACE, MIGRATIONS_DIR).await;
        let redis_endpoint = test_support::containers::redis_endpoint().await;

        let backends = Backends {
            scylla: ScyllaConfig {
                contact_points: vec![scylla_cp],
                keyspace:       None,
                ..ScyllaConfig::default()
            },
            redis: RedisConfig { hosts: vec![redis_endpoint], ..RedisConfig::default() },
        };

        let app = App::build(backends).await.expect("integration: build profile app");

        Self {
            command_bus: app.command_bus,
            query_bus:   app.query_bus,
            repository:  app.repository,
            cache:       app.cache,
        }
    }

    /// Creates a profile, expecting success.
    pub async fn create(&self, account_id: &str, handle: &str, display_name: &str) {
        dispatch_create(Arc::clone(&self.command_bus), account_id, handle, display_name)
            .await
            .expect("create_profile");
    }

    /// Updates a profile's display name.
    pub async fn update_display(&self, profile_id: &str, display_name: &str) {
        let cmd = UpdateProfileCommand {
            profile_id:   profile_id.to_owned(),
            display_name: Some(display_name.to_owned()),
            bio:          None,
            website_url:  None,
            locale:       None,
            custom_links: Vec::new(),
        };
        self.command_bus
            .dispatch(Envelope::new(Uuid::now_v7(), cmd))
            .await
            .expect("update_profile");
    }

    /// Resolves a profile by handle.
    pub async fn get_by_handle(&self, handle: &str) -> Option<ProfileView> {
        self.query_bus
            .dispatch(Envelope::new(Uuid::now_v7(), GetProfileByHandleQuery { handle: handle.to_owned() }))
            .await
            .expect("get_profile_by_handle")
    }

    /// Resolves a profile by id (read-through: warms the cache on a miss).
    pub async fn get_by_id(&self, profile_id: &str) -> Option<ProfileView> {
        self.query_bus
            .dispatch(Envelope::new(Uuid::now_v7(), GetProfileByIdQuery { profile_id: profile_id.to_owned() }))
            .await
            .expect("get_profile_by_id")
    }
}

/// Dispatches a create on a shared bus — a free function so scenarios can fire
/// many concurrently from spawned tasks.
pub async fn dispatch_create(
    command_bus:  Arc<InMemoryCommandBus>,
    account_id:   &str,
    handle:       &str,
    display_name: &str,
) -> Result<(), CqrsError> {
    let cmd = CreateProfileCommand {
        account_id:   account_id.to_owned(),
        handle:       handle.to_owned(),
        display_name: display_name.to_owned(),
        bio:          None,
        avatar_url:   None,
        banner_url:   None,
        profile_kind: "personal".to_owned(),
        locale:       "en-US".to_owned(),
    };
    command_bus.dispatch(Envelope::new(Uuid::now_v7(), cmd)).await
}

/// A fresh random account id (UUID string).
pub fn random_account_id() -> String {
    Uuid::now_v7().to_string()
}

/// A fresh random handle: `u` + 12 hex chars — valid (2–30 chars, starts/ends
/// alphanumeric, no disallowed characters).
pub fn random_handle() -> String {
    format!("u{}", &Uuid::now_v7().simple().to_string()[..12])
}
