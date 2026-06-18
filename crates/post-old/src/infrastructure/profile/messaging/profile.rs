// crates/post/src/presentation/messaging/profile_handler.rs

use crate::services::ProfileProjectionOrchestrator;
use shared_kernel::core::{Error, Result};
use shared_kernel::messaging::EventEnvelope;
use shared_kernel::types::Region;
use shared_proto::profile::v1::ProfileSummaryDto;
use std::sync::Arc;

pub struct ProfileEventHandler {
    orchestrator: Arc<ProfileProjectionOrchestrator>,
    region: Region,
}

impl ProfileEventHandler {
    pub fn new(orchestrator: Arc<ProfileProjectionOrchestrator>, region: Region) -> Self {
        Self {
            orchestrator,
            region,
        }
    }

    pub async fn handle(&self, envelope: EventEnvelope) -> Result<()> {
        let payload_value = envelope.payload;
        let profile_dto: ProfileSummaryDto =
            serde_json::from_value(payload_value).map_err(|e| {
                Error::internal(format!(
                    "Failed to map envelope payload value to ProfileSummaryDto: {}",
                    e
                ))
            })?;

        let updated_at_ms = envelope
            .metadata
            .as_ref()
            .and_then(|meta| meta.get("timestamp_ms"))
            .and_then(|t| t.as_i64())
            // Fallback défensif sur le temps système actuel si le timestamp est absent
            .unwrap_or_else(|| chrono::Utc::now().timestamp_millis());

        // 2. Délégation à l'orchestrateur applicatif
        self.orchestrator
            .project_change(self.region, profile_dto, updated_at_ms)
            .await?;

        Ok(())
    }
}
