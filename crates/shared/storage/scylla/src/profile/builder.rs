use std::sync::Arc;
use std::time::Duration;

use scylla::client::execution_profile::ExecutionProfile;
use scylla::frame::types::{Consistency, SerialConsistency};
use scylla::policies::load_balancing::DefaultPolicy;
use scylla::policies::retry::DefaultRetryPolicy;
use scylla::policies::speculative_execution::{
    SimpleSpeculativeExecutionPolicy, SpeculativeExecutionPolicy,
};

/// Fluent builder for a scylla [`ExecutionProfile`].
///
/// Each call returns `Self` — the final [`ProfileBuilder::build`] consumes the
/// builder and produces an immutable [`ExecutionProfile`] that can be registered
/// in a [`ProfileRegistry`] or converted directly to an
/// [`ExecutionProfileHandle`].
///
/// [`ProfileRegistry`]: super::registry::ProfileRegistry
/// [`ExecutionProfileHandle`]: scylla::client::execution_profile::ExecutionProfileHandle
pub struct ProfileBuilder {
    local_dc: String,
    consistency: Consistency,
    serial_consistency: Option<SerialConsistency>,
    request_timeout: Option<Duration>,
    permit_dc_failover: bool,
    speculative: Option<(usize, Duration)>,
}

impl ProfileBuilder {
    /// Starts a builder scoped to `local_dc`.
    ///
    /// The resulting profile uses token-aware + DC-aware routing via
    /// [`DefaultPolicy`], preferring nodes in `local_dc` and disabling
    /// cross-DC failover by default.
    pub fn new(local_dc: impl Into<String>) -> Self {
        Self {
            local_dc: local_dc.into(),
            consistency: Consistency::LocalQuorum,
            serial_consistency: Some(SerialConsistency::LocalSerial),
            request_timeout: Some(Duration::from_secs(5)),
            permit_dc_failover: false,
            speculative: None,
        }
    }

    pub fn consistency(mut self, c: Consistency) -> Self {
        self.consistency = c;
        self
    }

    pub fn serial_consistency(mut self, sc: Option<SerialConsistency>) -> Self {
        self.serial_consistency = sc;
        self
    }

    pub fn request_timeout(mut self, timeout: Option<Duration>) -> Self {
        self.request_timeout = timeout;
        self
    }

    pub fn permit_dc_failover(mut self, permit: bool) -> Self {
        self.permit_dc_failover = permit;
        self
    }

    /// Enables [`SimpleSpeculativeExecutionPolicy`]: fires up to `max_extra_attempts`
    /// additional speculative requests `delay` milliseconds apart before a
    /// response arrives from the primary coordinator.
    pub fn speculative_execution(mut self, max_extra_attempts: usize, delay: Duration) -> Self {
        self.speculative = Some((max_extra_attempts, delay));
        self
    }

    /// Builds an immutable [`ExecutionProfile`].
    ///
    /// The load-balancing policy is always [`DefaultPolicy`] (token-aware,
    /// DC-aware, no cross-DC failover unless [`Self::permit_dc_failover`] is
    /// set). Retry policy is always [`DefaultRetryPolicy`].
    pub fn build(self) -> ExecutionProfile {
        let lbp = DefaultPolicy::builder()
            .prefer_datacenter(self.local_dc)
            .token_aware(true)
            .permit_dc_failover(self.permit_dc_failover)
            .build();

        let speculative_policy: Option<Arc<dyn SpeculativeExecutionPolicy>> =
            self.speculative.map(|(max_retry_count, retry_interval)| {
                Arc::new(SimpleSpeculativeExecutionPolicy {
                    max_retry_count,
                    retry_interval,
                }) as Arc<dyn SpeculativeExecutionPolicy>
            });

        ExecutionProfile::builder()
            .consistency(self.consistency)
            .serial_consistency(self.serial_consistency)
            .request_timeout(self.request_timeout)
            .load_balancing_policy(lbp)
            .retry_policy(Arc::new(DefaultRetryPolicy::new()))
            .speculative_execution_policy(speculative_policy)
            .build()
    }
}
