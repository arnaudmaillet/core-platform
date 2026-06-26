//! Live, container-backed integration suite for the realtime delivery service.
//!
//! Realtime's whole point is the internal→external bridge — resolve a recipient,
//! hop to the owning node over Redis sharded Pub/Sub, and land the event on the
//! right socket — so this suite boots real **Redis** and drives that bridge over
//! the production adapters (`RedisConnectionRegistry`, `RedisNodeChannel`) plus the
//! node-local `ConnectionTable`. It exercises exactly what cannot be unit-tested:
//! that `HSET`/`HGETALL`/`PEXPIRE` round-trip the registry (and TTL self-heals a
//! leaked entry), that `SPUBLISH`→`SSUBSCRIBE` carries a prost `DeliverEnvelope`
//! intact, and that a fanned-out event reaches a subscribed connection's queue
//! while an unsubscribed/offline one gets nothing.
//!
//! The WebSocket accept loop and the auth/JWKS decoder are out of scope here (they
//! need a live IdP + a browser-grade WS client); their logic is unit-tested over
//! fakes, and the `spawn_node_subscriber` loop is the thin glue around the tested
//! `ConnectionTable::deliver` + decode path.
//!
//! Gated behind `integration-realtime` so the default `cargo test -p realtime`
//! stays hermetic and Docker-free:
//!
//! ```text
//! cargo test -p realtime --features integration-realtime -- --nocapture
//! ```
#![cfg(feature = "integration-realtime")]

mod realtime_it;
