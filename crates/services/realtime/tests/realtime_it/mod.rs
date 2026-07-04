//! Realtime integration suite: the harness (boots Redis and wires the real
//! routing adapters) and the bridge scenarios. Isolation is by fresh per-scenario
//! user ids (UUID); the shared container runs every scenario in parallel.

mod harness;
mod scenarios;
