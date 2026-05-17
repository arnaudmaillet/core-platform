// crates/profile/src/infrastructure/kafka/consumers/account_consumer.rs

// crates/profile/src/infrastructure/kafka/consumers/account_consumer.rs

use crate::application::context::{ProfileAppContext, ProfileContext};
use crate::commands::CreateProfileCommand;
use crate::types::Handle;
use serde::Deserialize;
use shared_kernel::core::Identifier;
use shared_kernel::{
    command::CommandBus,
    core::ErrorCode,
    types::{AccountId, RegionCode},
};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(tag = "type", content = "data")]
enum AccountIncomingEvent {
    #[serde(rename = "AccountRegistered")]
    Registered {
        account_id: Uuid,
        region: String,
    },
    #[serde(other)]
    Ignored,
}

pub struct AccountConsumer {
    bus: Arc<CommandBus>,
    app_ctx: ProfileAppContext,
}

impl AccountConsumer {
    pub fn new(bus: Arc<CommandBus>, app_ctx: ProfileAppContext) -> Self {
        Self { bus, app_ctx }
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
                // 💡 LOGIQUE D'AUTOGÉNÉRATION DU HANDLE
                // On génère un handle temporaire unique basé par exemple sur les 8 premiers caractères de l'UUID
                let short_id = &account_id.to_string()[0..8];
                let default_username = format!("user_{}", short_id);

                let handle = Handle::try_new(default_username)
                    .map_err(|e| format!("Failed to generate default handle: {}", e))?;

                let region_vo = RegionCode::try_new(region).map_err(|e| e.to_string())?;
                let creation_ctx = self.app_ctx.create_creation_context(region_vo.clone());

                let command = CreateProfileCommand {
                    command_id: Uuid::new_v4(),
                    account_id: AccountId::from_uuid(account_id),
                    handle, // 💡 Passé proprement au validateur de commande
                    region: region_vo,
                };

                match self
                    .bus
                    .execute::<ProfileContext, CreateProfileCommand, ()>(creation_ctx, command)
                    .await
                {
                    Ok(_) => {
                        tracing::info!(account_id = %account_id, "Default profile created successfully from AccountRegistered event");
                        Ok(())
                    }
                    Err(e) if e.code == ErrorCode::AlreadyExists => {
                        tracing::info!(account_id = %account_id, "Profile already initialized, skipping idempotently");
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
