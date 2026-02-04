// crates/profile/src/infrastructure/kafka/consumers/account_consumer.rs

use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;
use crate::application::create_profile::{CreateProfileUseCase, CreateProfileCommand};
use shared_kernel::domain::value_objects::{AccountId, RegionCode, Username};
use crate::domain::value_objects::DisplayName;

/// Le contrat local : on ne définit que ce qui nous intéresse.
#[derive(Deserialize)]
#[serde(tag = "type", content = "data")]
enum AccountIncomingEvent {
    #[serde(rename = "account.created")]
    Created {
        account_id: Uuid,
        region: String,
        username: String,
        display_name: String,
    },
    // Très important : capture tous les autres événements pour ne pas planter
    #[serde(other)]
    Ignored,
}

pub struct AccountConsumer {
    use_case: Arc<CreateProfileUseCase>,
}

impl AccountConsumer {
    pub fn new(use_case: Arc<CreateProfileUseCase>) -> Self {
        Self { use_case }
    }

    pub async fn on_message_received(&self, payload: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
        // 1. On vérifie d'abord si c'est un JSON valide
        // S'il est corrompu, on remonte l'erreur via ?
        let raw_value: serde_json::Value = serde_json::from_slice(payload)?;

        // 2. On tente de mapper vers notre enum
        // Si ça échoue ici, c'est que c'est un event qu'on ne gère pas (Ignored)
        let event: AccountIncomingEvent = match serde_json::from_value(raw_value) {
            Ok(e) => e,
            Err(_) => return Ok(()), // C'est ici qu'on ignore les events inconnus
        };

        match event {
            AccountIncomingEvent::Created { account_id, region, username, display_name } => {
                let command = CreateProfileCommand {
                    account_id: AccountId::from(account_id),
                    region: RegionCode::try_new(region).map_err(|e| e.to_string())?,
                    username: Username::try_new(username.clone()).map_err(|e| e.to_string())?,
                    display_name: match DisplayName::try_new(display_name) {
                        Ok(vo) => vo,
                        Err(_) => DisplayName::try_new(username).map_err(|e| e.to_string())?,
                    },
                };

                match self.use_case.execute(command).await {
                    Ok(_) => Ok(()),
                    Err(shared_kernel::errors::DomainError::AlreadyExists { .. }) => Ok(()),
                    Err(e) => Err(Box::new(e)),
                }
            },
            AccountIncomingEvent::Ignored => Ok(()),
        }
    }
}