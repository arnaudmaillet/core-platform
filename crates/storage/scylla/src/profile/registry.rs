use std::time::Duration;

use scylla::client::execution_profile::ExecutionProfile;
use scylla::frame::types::Consistency;

use super::builder::ProfileBuilder;

/// Discriminant for the three built-in execution profiles.
///
/// Each variant maps to a pre-configured [`ExecutionProfile`] held in a
/// [`ProfileRegistry`]. Upstream CQRS handlers select the appropriate profile
/// for each statement based on their consistency and latency requirements.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProfileKind {
    /// **LocalQuorum** consistency. No speculative execution. 5 s timeout.
    ///
    /// Intended for mutation handlers (writes to feeds, follow graph insertions)
    /// where durability and linearizability within the local datacenter are
    /// required. The stricter consistency avoids cross-DC coordination overhead.
    Strict,

    /// **LocalOne** consistency with `SimpleSpeculativeExecution` (1 extra
    /// attempt, 50 ms delay). 2 s timeout.
    ///
    /// Intended for read-heavy, tail-latency-sensitive paths (timeline reads,
    /// feed lookups, activity streams). Fires a speculative backup request to a
    /// second replica if the primary coordinator stalls, masking a node hiccup
    /// without a full round-trip penalty.
    Fast,

    /// **Quorum** consistency. No speculative execution. 30 s timeout.
    ///
    /// Intended for background aggregation jobs and admin-level reads that can
    /// tolerate higher latency but require cross-replica agreement (e.g.
    /// computing follower counts for analytics pipelines). The extended timeout
    /// accommodates heavy-scan queries.
    Analytical,
}

/// Immutable registry of named [`ExecutionProfile`]s.
///
/// Built once at application startup via [`ProfileRegistry::new`], then passed
/// to [`ScyllaSessionBuilder`] which registers the `Strict` profile as the
/// session default and exposes the registry to upstream services for
/// per-statement profile selection.
///
/// [`ScyllaSessionBuilder`]: super::super::session::builder::ScyllaSessionBuilder
pub struct ProfileRegistry {
    strict:     ExecutionProfile,
    fast:       ExecutionProfile,
    analytical: ExecutionProfile,
}

impl ProfileRegistry {
    /// Constructs the three standard profiles for the given `local_dc`.
    ///
    /// All profiles use token-aware + DC-aware load balancing with
    /// `local_dc` as the preferred datacenter and DC-failover disabled.
    pub fn new(local_dc: impl Into<String> + Clone) -> Self {
        let strict = ProfileBuilder::new(local_dc.clone())
            .consistency(Consistency::LocalQuorum)
            .request_timeout(Some(Duration::from_secs(5)))
            .build();

        let fast = ProfileBuilder::new(local_dc.clone())
            .consistency(Consistency::LocalOne)
            .serial_consistency(None)
            .request_timeout(Some(Duration::from_millis(2_000)))
            .speculative_execution(1, Duration::from_millis(50))
            .build();

        let analytical = ProfileBuilder::new(local_dc)
            .consistency(Consistency::Quorum)
            .serial_consistency(None)
            .request_timeout(Some(Duration::from_secs(30)))
            .build();

        Self { strict, fast, analytical }
    }

    /// Returns the `Strict` profile (LocalQuorum, no speculative execution).
    pub fn strict(&self) -> &ExecutionProfile {
        &self.strict
    }

    /// Returns the `Fast` profile (LocalOne + speculative execution).
    pub fn fast(&self) -> &ExecutionProfile {
        &self.fast
    }

    /// Returns the `Analytical` profile (Quorum, 30 s timeout).
    pub fn analytical(&self) -> &ExecutionProfile {
        &self.analytical
    }

    /// Returns the profile for `kind`.
    pub fn get(&self, kind: ProfileKind) -> &ExecutionProfile {
        match kind {
            ProfileKind::Strict     => &self.strict,
            ProfileKind::Fast       => &self.fast,
            ProfileKind::Analytical => &self.analytical,
        }
    }
}
