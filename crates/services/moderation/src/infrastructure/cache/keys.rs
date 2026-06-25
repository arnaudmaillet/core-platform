//! Redis key builders. Single-key operations, but every key carries a `{…}` hash
//! tag on its entity so any future multi-key script stays slot-safe on Redis
//! Cluster (the mandatory slot-safety rule).

use crate::domain::value_object::ActorId;

/// Hot-path enforcement flag for an actor: `mod:enf:{<actor>}`.
pub fn enforcement_key(actor: &ActorId) -> String {
    format!("mod:enf:{{{actor}}}")
}

/// Known-bad corpus entry for a content hash: `mod:corpus:{<algo>}:<value>`.
pub fn corpus_key(algorithm: &str, value: &str) -> String {
    format!("mod:corpus:{{{algorithm}}}:{value}")
}
