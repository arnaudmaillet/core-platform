//! Runtime traffic values, the keying/mode enums, and the limiter decision type.

use std::time::Duration;

/// Key dimension a profile limits on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum Scope {
    /// One bucket per gRPC method — fleet-coarse, identity-free, enforceable pre-auth.
    #[default]
    PerMethod,
    /// One bucket per authenticated caller per method. Requires an upstream layer to have
    /// established the principal; falls back to method-level keying when none is present.
    PerCaller,
}

/// Where the limiter's counter state lives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum Mode {
    /// In-process, per-replica. The only mode enforced today.
    #[default]
    Local,
    /// Redis-coordinated global lease. Parsed for forward-compatibility but not yet
    /// enforced — `infra-config` validation rejects it until Step 2 ships the backend.
    Distributed,
}

/// What a distributed profile does when its coordination backend is unreachable.
/// Parsed now so adding the distributed backend needs no schema migration; inert until then.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum BackendError {
    /// Degrade to the always-on local limiter (availability over precision).
    #[default]
    FailOpen,
    /// Reject (precision/safety over availability) — for hard abuse/billing quotas.
    FailClosed,
}

/// Resolved, runtime traffic values for one profile. Cheap to clone and compare.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrafficConfig {
    /// Sustained admit rate, requests per second. Always `> 0` once validated.
    pub rps: u32,
    /// Bucket capacity — the largest instantaneous burst admitted. Always `>= 1`.
    pub burst: u32,
    /// Key dimension.
    pub scope: Scope,
    /// State-locality mode.
    pub mode: Mode,
    /// Distributed-only (Step 2): replica↔backend lease sync cadence, milliseconds.
    pub lease_ms: Option<u64>,
    /// Distributed-only (Step 2): backend-failure policy.
    pub on_backend_error: Option<BackendError>,
}

/// Outcome of a limiter check on the hot path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrafficDecision {
    /// Admit the request.
    Allow,
    /// Shed the request; `retry_after` is the soonest a retry under this key could succeed.
    Throttle { retry_after: Duration },
}
