// crates/account/src/infrastructure/postgres/repositories/mock_global_registry.rs

use account_old::repositories::{GlobalIdentityRegistration, GlobalIdentityRegistry};
use account_old::types::{AccountState, RegistrationIdentifier};
use async_trait::async_trait;
use shared_kernel::core::{Error, Result};
use shared_kernel::types::AccountId;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone, Default)]
pub struct GlobalIdentityRegistryStub {
    records: Arc<RwLock<HashMap<AccountId, GlobalIdentityRegistration>>>,
}

impl GlobalIdentityRegistryStub {
    pub fn new() -> Self {
        Self {
            records: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn insert_fixture(&self, registration: GlobalIdentityRegistration) {
        let mut guard = self.records.write().await;
        guard.insert(registration.account_id, registration);
    }
}

#[async_trait]
impl GlobalIdentityRegistry for GlobalIdentityRegistryStub {
    async fn reserve(&self, registration: &GlobalIdentityRegistration) -> Result<()> {
        let mut guard = self.records.write().await;

        // SIMULATION DES INDEX UNIQUES POSTGRES (Contraintes d'unicité globales)
        for existing in guard.values() {
            // 1. Contrainte de doublon d'email
            if let (Some(reg_email), Some(exist_email)) = (
                registration.identifiers.email_hash(),
                existing.identifiers.email_hash(),
            ) {
                if reg_email == exist_email {
                    return Err(Error::validation(
                        "email",
                        "This email address is already registered globally.",
                    ));
                }
            }

            // 2. Contrainte de doublon de téléphone
            if let (Some(reg_phone), Some(exist_phone)) = (
                registration.identifiers.phone_hash(),
                existing.identifiers.phone_hash(),
            ) {
                if reg_phone == exist_phone {
                    return Err(Error::validation(
                        "phone_number",
                        "This phone number is already registered globally.",
                    ));
                }
            }

            // 3. Contrainte de doublon sur le sub_id Keycloak
            if registration.sub_id.is_some() && registration.sub_id == existing.sub_id {
                return Err(Error::validation(
                    "sub_id",
                    "This external identity provider sub_id is already linked to an account.",
                ));
            }
        }

        guard.insert(registration.account_id, registration.clone());
        Ok(())
    }

    async fn find_by_account_id(
        &self,
        account_id: AccountId,
    ) -> Result<Option<GlobalIdentityRegistration>> {
        let guard = self.records.read().await;
        Ok(guard.get(&account_id).cloned())
    }

    async fn find_by_email_hash(
        &self,
        email_hash: &[u8],
    ) -> Result<Option<GlobalIdentityRegistration>> {
        let guard = self.records.read().await;
        let found = guard
            .values()
            .find(|r| {
                // 💡 On passe par l'accesseur .email_hash() de l'objet identifiers
                r.identifiers.email_hash().as_deref() == Some(email_hash)
            })
            .cloned();
        Ok(found)
    }

    async fn find_by_phone_hash(
        &self,
        phone_hash: &[u8],
    ) -> Result<Option<GlobalIdentityRegistration>> {
        let guard = self.records.read().await;
        let found = guard
            .values()
            .find(|r| {
                // 💡 On passe par l'accesseur .phone_hash() de l'objet identifiers
                r.identifiers.phone_hash().as_deref() == Some(phone_hash)
            })
            .cloned();
        Ok(found)
    }

    async fn find_by_sub_id(&self, sub_id: &str) -> Result<Option<GlobalIdentityRegistration>> {
        let guard = self.records.read().await;
        let found = guard
            .values()
            .find(|r| r.sub_id.as_ref().map(|s| s.as_str()) == Some(sub_id))
            .cloned();
        Ok(found)
    }

    async fn update_identifiers(
        &self,
        account_id: AccountId,
        new_identifiers: RegistrationIdentifier,
    ) -> Result<()> {
        let mut guard = self.records.write().await;

        if !guard.contains_key(&account_id) {
            return Err(Error::database(format!(
                "Global identity record not found for update: {}",
                account_id
            )));
        }

        // SIMULATION DU VÉRIFICATEUR D'UNICITÉ LORS D'UN UPDATE
        for (existing_id, existing) in guard.iter() {
            if existing_id == &account_id {
                continue; // On ne se compare pas avec soi-même
            }

            // Validation de l'email
            if let (Some(new_email), Some(exist_email)) = (
                new_identifiers.email_hash(),
                existing.identifiers.email_hash(),
            ) {
                if new_email == exist_email {
                    return Err(Error::validation(
                        "email",
                        "This email address is already claimed.",
                    ));
                }
            }

            if let (Some(new_phone), Some(exist_phone)) = (
                new_identifiers.phone_hash(),
                existing.identifiers.phone_hash(),
            ) {
                if new_phone == exist_phone {
                    return Err(Error::validation(
                        "phone_number",
                        "This phone number is already claimed.",
                    ));
                }
            }
        }

        if let Some(record) = guard.get_mut(&account_id) {
            record.identifiers = new_identifiers;
        }

        Ok(())
    }

    async fn update_state(&self, account_id: AccountId, new_state: AccountState) -> Result<()> {
        let mut guard = self.records.write().await;

        if let Some(record) = guard.get_mut(&account_id) {
            record.state = new_state;
            Ok(())
        } else {
            Err(Error::database(format!(
                "Global identity record not found for state update: {}",
                account_id
            )))
        }
    }

    async fn delete(&self, account_id: AccountId) -> Result<()> {
        let mut guard = self.records.write().await;
        guard.remove(&account_id);
        Ok(())
    }

    async fn purge_expired_reservations(
        &self,
        expired_before: chrono::DateTime<chrono::Utc>,
    ) -> Result<u64> {
        let mut guard = self.records.write().await;
        let initial_count = guard.len();

        // On conserve uniquement ce qui n'est PAS en PENDING *ou* ce qui n'est pas encore expiré
        guard.retain(|_, registration| {
            !(registration.state == AccountState::PENDING
                && registration.created_at < expired_before)
        });

        let purged = (initial_count - guard.len()) as u64;
        Ok(purged)
    }
}
