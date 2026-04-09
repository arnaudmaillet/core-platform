// crates/account/src/application/link_external_identity/link_external_identity_use_case.rs

use crate::application::use_cases::access_management::link_external_identity::LinkExternalIdentityCommand;
use crate::domain::account::entities::AccountIdentity;
use crate::domain::repositories::AccountIdentityRepository;
use shared_kernel::domain::entities::EntityOptionExt;
use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::domain::repositories::OutboxRepository;
use shared_kernel::domain::transaction::TransactionManager;
use shared_kernel::domain::utils::{RetryConfig, with_retry};
use shared_kernel::errors::{DomainError, Result};
use shared_kernel::infrastructure::postgres::transactions::TransactionManagerExt;
use std::sync::Arc;

pub struct LinkExternalIdentityUseCase {
    identity_repo: Arc<dyn AccountIdentityRepository>,
    outbox_repo: Arc<dyn OutboxRepository>,
    tx_manager: Arc<dyn TransactionManager>,
}

impl LinkExternalIdentityUseCase {
    pub fn new(
        identity_repo: Arc<dyn AccountIdentityRepository>,
        outbox_repo: Arc<dyn OutboxRepository>,
        tx_manager: Arc<dyn TransactionManager>,
    ) -> Self {
        Self {
            identity_repo,
            outbox_repo,
            tx_manager,
        }
    }

    pub async fn execute(&self, command: LinkExternalIdentityCommand) -> Result<AccountIdentity> {
        with_retry(RetryConfig::default(), || async {
            self.try_execute_once(&command).await
        })
        .await
    }

    async fn try_execute_once(&self, cmd: &LinkExternalIdentityCommand) -> Result<AccountIdentity> {
        // 1. VÉRIFICATION D'UNICITÉ ET LECTURE OPTIMISTE (Hors transaction)
        
        // On utilise resolve_id_from_external_id pour vérifier si l'ID est déjà pris
        if let Some(existing_account_id) = self
            .identity_repo
            .resolve_id_from_external_id(&cmd.external_id)
            .await?
        {
            // Si l'ID appartient à un AUTRE compte : Erreur
            if existing_account_id != cmd.account_id {
                return Err(DomainError::AlreadyExists {
                    entity: "Account",
                    field: "external_id",
                    value: cmd.external_id.as_str().to_string(),
                });
            }
            
            // Idempotence : si c'est déjà lié à CE compte, on renvoie simplement l'état actuel
            return self.identity_repo
                .fetch_by_account_id(&cmd.account_id, None)
                .await?
                .ok_or_not_found(&cmd.account_id);
        }

        // On récupère le compte original pour la mutation et le verrouillage optimiste
        let original_identity = self
            .identity_repo
            .fetch_by_account_id(&cmd.account_id, None)
            .await?
            .ok_or_not_found(&cmd.account_id)?;

        let mut identity = original_identity.clone();

        // 2. MUTATION DU MODÈLE RICHE
        // link_external_identity renvoie false si l'ID était déjà identique (idempotence au niveau entité)
        if !identity.link_external_identity(cmd.external_id.clone())? {
            return Ok(original_identity);
        }
        
        // 3. EXTRACTION DES ÉVÉNEMENTS
        let events = identity.pull_events();
        if events.is_empty() {
             return Ok(identity);
        }

        // 4. PRÉPARATION DES DONNÉES POUR LA TRANSACTION
        let updated_identity = identity.clone();
        let identity_repo = Arc::clone(&self.identity_repo);
        let outbox_repo = Arc::clone(&self.outbox_repo);

        // 5. PERSISTANCE TRANSACTIONNELLE ATOMIQUE
        self.tx_manager
            .run_in_transaction(move |mut tx| {
                let identity_repo = Arc::clone(&identity_repo);
                let outbox_repo = Arc::clone(&outbox_repo);

                let original_for_tx = original_identity.clone();
                let updated_for_tx = identity.clone();
                let events_for_tx = events.clone();

                Box::pin(async move {
                    identity_repo.save(&updated_for_tx, Some(&original_for_tx), Some(&mut *tx))
                        .await?;

                    for event in events_for_tx {
                        outbox_repo.save(&mut *tx, event.as_ref()).await?;
                    }
                    tx.commit().await?;
                    Ok(())
                })
            })
            .await?;

        Ok(updated_identity)
    }
}