use crate::application::context::{ProfileKernelCtx, ProfileCommandCtx};
use crate::commands::CreateProfileCommand;
use crate::types::Handle;
use serde::Deserialize;
use shared_kernel::command::CommandTarget;
use shared_kernel::core::Identifier;
use shared_kernel::{
    command::CommandBus,
    core::ErrorCode,
    types::{AccountId, ProfileId, Region},
};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(tag = "type", content = "data")]
enum AccountIncomingEvent {
    #[serde(rename = "AccountRegistered")]
    Registered { account_id: Uuid, region: String },
    #[serde(other)]
    Ignored,
}

pub struct AccountConsumer {
    bus: Arc<CommandBus>,
    kernel: ProfileKernelCtx,
}

impl AccountConsumer {
    pub fn new(bus: Arc<CommandBus>, kernel: ProfileKernelCtx) -> Self {
        Self { bus, kernel }
    }

    pub async fn on_message_received(
        &self,
        payload: &[u8],
    ) -> Result<(), Box<dyn std::error::Error>> {
        let raw_value: serde_json::Value = serde_json::from_slice(payload)?;
        let event: AccountIncomingEvent = match serde_json::from_value(raw_value) {
            Ok(e) => e,
            Err(_) => return Ok(()),
        };

        match event {
            AccountIncomingEvent::Registered {
                account_id, region, ..
            } => {
                let region_vo = Region::try_from(region.as_str()).map_err(|e| e.to_string())?;

                if region_vo != self.kernel.server_region() {
                    tracing::debug!(
                        account_id = %account_id,
                        event_region = ?region_vo,
                        local_region = ?self.kernel.server_region(),
                        "Account registered in another region, skipping locally"
                    );
                    return Ok(());
                }

                let short_id = &account_id.to_string()[0..8];
                let default_username = format!("user_{}", short_id);

                let handle = Handle::try_new(default_username)
                    .map_err(|e| format!("Failed to generate default handle: {}", e))?;

                let creation_ctx = self.kernel.creation_command(region_vo);
                let generated_profile_id = ProfileId::generate();
                let target = CommandTarget::stateless(generated_profile_id);

                let command = CreateProfileCommand {
                    command_id: Uuid::new_v4(),
                    target,
                    region: region_vo,
                    account_id: AccountId::from_uuid(account_id),
                    handle,
                };

                match self
                    .bus
                    .execute::<ProfileCommandCtx, CreateProfileCommand, ()>(
                        creation_ctx,
                        command,
                    )
                    .await
                {
                    Ok(_) => {
                        tracing::info!(
                            account_id = %account_id,
                            profile_id = %generated_profile_id,
                            "Default profile created successfully from AccountRegistered event"
                        );
                        Ok(())
                    }
                    Err(e) if e.code == ErrorCode::AlreadyExists => {
                        tracing::info!(
                            account_id = %account_id,
                            "Profile already initialized, skipping idempotently"
                        );
                        Ok(())
                    }
                    Err(e) => {
                        tracing::error!("KAFKA CONSUMER EXECUTION ERROR: {:?}", e);
                        Err(Box::new(std::io::Error::new(
                            std::io::ErrorKind::Other,
                            e.message,
                        )))
                    }
                }
            }
            AccountIncomingEvent::Ignored => Ok(()),
        }
    }
}
