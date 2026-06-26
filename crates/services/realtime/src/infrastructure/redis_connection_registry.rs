//! The connection/presence registry over Redis (fred).
//!
//! Layout — one HASH per user, hash-tagged so all of a user's fields share a
//! cluster slot:
//! * `rt:presence:{<user_id>}` — field = `connection_id`, value = JSON
//!   `{device_id, node_id}`.
//!
//! Writes go through single-key Lua `eval` (the fleet pattern, Cluster-safe); the
//! `bind` script also refreshes a TTL so a leaked entry (a connection whose evict
//! never ran) self-heals. Reads use `HGETALL`. A fred fault on any path is
//! reported as `RTM-4001 RegistryUnavailable` (retryable) so the dispatcher's
//! `run_consumer` retries rather than dropping the event.

use std::collections::HashMap;

use async_trait::async_trait;
use fred::interfaces::{HashesInterface, LuaInterface};
use redis_storage::RedisClient;
use serde::{Deserialize, Serialize};

use crate::application::port::{ConnectionLocation, ConnectionRegistry};
use crate::domain::{ConnectionId, DeviceId, NodeId, UserId};
use crate::error::RealtimeError;

/// `HSET field value` then refresh the hash TTL; returns 1.
const BIND: &str = "redis.call('HSET', KEYS[1], ARGV[1], ARGV[2]); \
                    redis.call('PEXPIRE', KEYS[1], ARGV[3]); return 1";
/// `HDEL field`; returns the number removed.
const EVICT: &str = "return redis.call('HDEL', KEYS[1], ARGV[1])";

#[derive(Serialize, Deserialize)]
struct PlacementValue {
    device_id: String,
    node_id: String,
}

fn presence_key(user_id: &UserId) -> String {
    format!("rt:presence:{{{}}}", user_id.as_str())
}

fn registry_err(e: fred::error::Error) -> RealtimeError {
    tracing::warn!(error = %e, "connection registry redis error");
    RealtimeError::RegistryUnavailable
}

pub struct RedisConnectionRegistry {
    client: RedisClient,
    /// TTL applied to each presence hash on bind; a self-heal bound on leaked
    /// entries, refreshed on every bind/heartbeat-rebind.
    ttl_ms: i64,
}

impl RedisConnectionRegistry {
    pub fn new(client: RedisClient, ttl_ms: i64) -> Self {
        Self { client, ttl_ms }
    }
}

#[async_trait]
impl ConnectionRegistry for RedisConnectionRegistry {
    async fn bind(&self, location: &ConnectionLocation) -> Result<(), RealtimeError> {
        let value = serde_json::to_string(&PlacementValue {
            device_id: location.device_id.as_str().to_owned(),
            node_id: location.node_id.as_str().to_owned(),
        })
        .map_err(|e| RealtimeError::DomainViolation {
            field: "placement".to_owned(),
            message: e.to_string(),
        })?;

        let _: i64 = self
            .client
            .inner
            .eval(
                BIND,
                vec![presence_key(&location.user_id)],
                vec![
                    location.connection_id.as_str().to_owned(),
                    value,
                    self.ttl_ms.to_string(),
                ],
            )
            .await
            .map_err(registry_err)?;
        Ok(())
    }

    async fn evict(
        &self,
        user_id: &UserId,
        connection_id: &ConnectionId,
    ) -> Result<(), RealtimeError> {
        let _: i64 = self
            .client
            .inner
            .eval(
                EVICT,
                vec![presence_key(user_id)],
                vec![connection_id.as_str().to_owned()],
            )
            .await
            .map_err(registry_err)?;
        Ok(())
    }

    async fn resolve(&self, user_id: &UserId) -> Result<Vec<ConnectionLocation>, RealtimeError> {
        let fields: HashMap<String, String> = self
            .client
            .inner
            .hgetall(presence_key(user_id))
            .await
            .map_err(registry_err)?;

        let mut out = Vec::with_capacity(fields.len());
        for (connection_id, raw) in fields {
            // A malformed entry is skipped, not fatal — the registry is best-effort
            // routing metadata, and a bad row self-heals on the next bind/TTL.
            let Ok(value) = serde_json::from_str::<PlacementValue>(&raw) else {
                continue;
            };
            let (Ok(device_id), Ok(node_id), Ok(connection_id)) = (
                DeviceId::new(value.device_id),
                NodeId::new(value.node_id),
                ConnectionId::new(connection_id),
            ) else {
                continue;
            };
            out.push(ConnectionLocation {
                user_id: user_id.clone(),
                device_id,
                connection_id,
                node_id,
            });
        }
        Ok(out)
    }
}
