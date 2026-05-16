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
    #[serde(rename = "account.created")]
    Created {
        account_id: Uuid,
        region: String,
        username: String,
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
        // 1. Désérialisation globale du payload
        let raw_value: serde_json::Value = serde_json::from_slice(payload)?;
        let event: AccountIncomingEvent = match serde_json::from_value(raw_value) {
            Ok(e) => e,
            Err(_) => return Ok(()),
        };

        match event {
            AccountIncomingEvent::Created {
                account_id,
                region,
                username,
                ..
            } => {
                // 2. Application de la règle métier : main handle = account username
                let handle = Handle::try_new(username.clone())
                    .map_err(|e| format!("Invalid handle from account username: {}", e))?;

                let region_vo = RegionCode::try_new(region).map_err(|e| e.to_string())?;

                // 3. Génération du contexte de création
                let creation_ctx = self.app_ctx.create_creation_context(region_vo.clone());

                // 4. Préparation de la commande avec son ID technique d'idempotence
                let command = CreateProfileCommand {
                    command_id: Uuid::new_v4(), // Idempotence technique générée à la volée pour le Bus
                    account_id: AccountId::from_uuid(account_id),
                    handle,
                    region: region_vo,
                };

                // 5. Exécution via le CommandBus en passant explicitement le contexte de création
                match self
                    .bus
                    .execute::<ProfileContext, CreateProfileCommand, ()>(creation_ctx, command)
                    .await
                {
                    Ok(_) => {
                        tracing::info!(account_id = %account_id, "Profile created successfully from Kafka event");
                        Ok(())
                    }
                    // Idempotence Business/Réseau : Si le message Kafka est rejoué, le handle ou l'ID sera détecté
                    // comme déjà existant. C'est un comportement attendu, on acquitte (Ok(())) sans planter.
                    Err(e) if e.code == ErrorCode::AlreadyExists => {
                        tracing::info!(
                            account_id = %account_id,
                            "Profile or handle already processed, skipping idempotently"
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
