//! Integration harness: boots the shared infra, wires a real geo-discovery graph
//! against it through the production composition root, and exposes the buses for
//! assertions. Indexing is driven through the command bus (no Kafka); reads go
//! through the viewport query.
#![allow(dead_code)]

use std::sync::Arc;
use std::time::Duration;

use uuid::Uuid;

use cqrs::command::InMemoryCommandBus;
use cqrs::query::InMemoryQueryBus;
use cqrs::{CommandBus, Envelope, QueryBus};
use redis_storage::RedisConfig;
use scylla_storage::ScyllaConfig;

use geo_discovery::app::{App, Backends};
use geo_discovery::application::command::IndexPostCommand;
use geo_discovery::application::query::get_geo_timeline::{GetGeoTimelineQuery, GetGeoTimelineResult};
use geo_discovery::application::query::query_tile::{QueryTileQuery, QueryTileResult};
use geo_discovery::config::GeoDiscoveryConfig;

pub use test_support::await_until;

/// Generous default patience for a cross-component assertion (ScyllaDB +
/// Redis spatial-index write visibility).
pub const DEADLINE: Duration = Duration::from_secs(10);

/// ScyllaDB keyspace the migrations provision.
const KEYSPACE: &str = "geo_discovery";
/// On-disk migration assets, resolved against *this* crate's manifest.
const MIGRATIONS_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/migrations");

/// Zoom 15 → H3 R9, whose virality floor is 0 — so the spatial filter, not a
/// score threshold, governs what a query returns.
pub const ZOOM_R9: i32 = 15;

/// A fully-wired geo-discovery service bound to ephemeral infra, plus the buses.
pub struct TestHarness {
    pub command_bus: Arc<InMemoryCommandBus>,
    pub query_bus:   Arc<InMemoryQueryBus>,
}

impl TestHarness {
    /// Boots/reuses the shared containers, applies migrations, and assembles the
    /// service graph (no Kafka workers).
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
            kafka: None,
        };

        let app = App::build(GeoDiscoveryConfig::from_env(), backends)
            .await
            .expect("integration: build geo-discovery app");

        Self { command_bus: app.command_bus, query_bus: app.query_bus }
    }

    /// Indexes a post at `(lat, lng)` with the given virality, returning its uuid.
    pub async fn index_post(&self, lat: f64, lng: f64, virality: f64) -> Uuid {
        self.index_post_full(lat, lng, virality, "", "").await
    }

    /// Indexes a post with an explicit caption and thumbnail — exercises the
    /// Focus (GetGeoTimeline) hydration path.
    pub async fn index_post_full(
        &self,
        lat:       f64,
        lng:       f64,
        virality:  f64,
        caption:   &str,
        thumbnail: &str,
    ) -> Uuid {
        let post_uuid = Uuid::now_v7();
        let cmd = IndexPostCommand {
            post_id:           post_uuid.to_string(),
            author_id:         Uuid::now_v7().to_string(),
            author_handle:     "tester".to_owned(),
            author_avatar_url: String::new(),
            thumbnail_url:     thumbnail.to_owned(),
            caption:           caption.to_owned(),
            lat,
            lng,
            virality_score:    virality,
            published_at_ms:   1_000,
            retention_secs:    None,
            author_tier:       0,
        };
        self.command_bus
            .dispatch(Envelope::new(Uuid::now_v7(), cmd))
            .await
            .expect("index_post");
        post_uuid
    }

    /// Focus path: hydrates the given post ids into full cards.
    pub async fn get_timeline(&self, post_ids: &[Uuid]) -> GetGeoTimelineResult {
        self.query_bus
            .dispatch(Envelope::new(
                Uuid::now_v7(),
                GetGeoTimelineQuery { post_ids: post_ids.to_vec() },
            ))
            .await
            .expect("get_geo_timeline")
    }

    /// Queries a viewport box (`sw` < `ne`) at the given zoom.
    pub async fn query_viewport(
        &self,
        sw_lat: f64,
        sw_lng: f64,
        ne_lat: f64,
        ne_lng: f64,
        zoom:   i32,
    ) -> QueryTileResult {
        self.query_bus
            .dispatch(Envelope::new(
                Uuid::now_v7(),
                QueryTileQuery { sw_lat, sw_lng, ne_lat, ne_lng, zoom_level: zoom },
            ))
            .await
            .expect("query_tile")
    }
}

/// Whether a Radar query result contains a pin for `post_uuid`.
pub fn result_contains(result: &QueryTileResult, post_uuid: &Uuid) -> bool {
    result.pins.iter().any(|p| p.post_id == *post_uuid)
}
