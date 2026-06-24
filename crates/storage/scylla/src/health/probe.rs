//! Ready-made [`HealthProbe`] over a ScyllaDB client, so services don't hand-roll
//! the readiness closure.

use std::sync::Arc;

use async_trait::async_trait;
use health::HealthProbe;

use crate::ScyllaClient;

struct ScyllaHealthProbe {
    client: Arc<ScyllaClient>,
}

#[async_trait]
impl HealthProbe for ScyllaHealthProbe {
    fn name(&self) -> &str {
        "scylla"
    }

    async fn check(&self) -> anyhow::Result<()> {
        super::check::health_check(&self.client.session)
            .await
            .map_err(|e| anyhow::anyhow!("scylla: {e}"))
    }
}

/// Builds a readiness probe (name `"scylla"`) over a live ScyllaDB client.
pub fn probe(client: Arc<ScyllaClient>) -> Arc<dyn HealthProbe> {
    Arc::new(ScyllaHealthProbe { client })
}
