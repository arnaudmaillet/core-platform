use crate::application::context::{ProfileAppContext, ProfileCommandContext};
use crate::commands::CreateProfileCommand;
use crate::types::Handle;
use serde::Deserialize;
use shared_kernel::command::CommandTarget;
use shared_kernel::core::{Identifier, TransactionManager};
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

pub struct AccountConsumer<TM> {
    bus: Arc<CommandBus>,
    app_ctx: ProfileAppContext<TM>,
}

impl<TM> AccountConsumer<TM> {
    pub fn new(bus: Arc<CommandBus>, app_ctx: ProfileAppContext<TM>) -> Self {
        Self { bus, app_ctx }
    }
}

impl<TM: TransactionManager + Clone + 'static> AccountConsumer<TM> {
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
                let short_id = &account_id.to_string()[0..8];
                let default_username = format!("user_{}", short_id);

                let handle = Handle::try_new(default_username)
                    .map_err(|e| format!("Failed to generate default handle: {}", e))?;

                let region_vo = Region::try_new(region).map_err(|e| e.to_string())?;
                let creation_ctx = self.app_ctx.creation_command(region_vo.clone());
                let generated_profile_id = ProfileId::generate();
                let target = CommandTarget::stateless(generated_profile_id, region_vo);

                let command = CreateProfileCommand {
                    command_id: Uuid::new_v4(),
                    target,
                    account_id: AccountId::from_uuid(account_id),
                    handle,
                };

                match self
                    .bus
                    .execute::<ProfileCommandContext<TM>, CreateProfileCommand, ()>(
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
