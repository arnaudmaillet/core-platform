// crates/account/src/application/link_external_identity/link_external_identity_use_case.rs

// crates/account/src/application/link_external_identity/link_external_identity_use_case.rs

use crate::application::use_cases::link_external_identity::LinkExternalIdentityCommand;
use crate::domain::account::entities::Account;
use crate::domain::repositories::AccountRepository;
use shared_kernel::domain::entities::EntityOptionExt;
use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::domain::repositories::OutboxRepository;
use shared_kernel::domain::transaction::TransactionManager;
use shared_kernel::domain::utils::{RetryConfig, with_retry};
use shared_kernel::errors::{DomainError, Result};
use shared_kernel::infrastructure::postgres::transactions::TransactionManagerExt;
use std::sync::Arc;

pub struct LinkExternalIdentityUseCase {
    repo: Arc<dyn AccountRepository>,
    outbox: Arc<dyn OutboxRepository>,
    tx_manager: Arc<dyn TransactionManager>,
}

impl LinkExternalIdentityUseCase {
    pub fn new(
        repo: Arc<dyn AccountRepository>,
        outbox: Arc<dyn OutboxRepository>,
        tx_manager: Arc<dyn TransactionManager>,
    ) -> Self {
        Self {
            repo,
            outbox,
            tx_manager,
        }
    }

    pub async fn execute(&self, command: LinkExternalIdentityCommand) -> Result<Account> {
        with_retry(RetryConfig::default(), || async {
            self.try_execute_once(&command).await
        })
        .await
    }

    async fn try_execute_once(&self, cmd: &LinkExternalIdentityCommand) -> Result<Account> {
        // 1. VÉRIFICATION D'UNICITÉ ET LECTURE OPTIMISTE (Hors transaction)
        
        // On utilise resolve_id_from_external_id pour vérifier si l'ID est déjà pris
        if let Some(existing_account_id) = self
            .repo
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
            return self.repo
                .fetch_by_id(&cmd.account_id, None)
                .await?
                .ok_or_not_found(&cmd.account_id);
        }

        // On récupère le compte original pour la mutation et le verrouillage optimiste
        let original_account = self
            .repo
            .fetch_by_id(&cmd.account_id, None)
            .await?
            .ok_or_not_found(&cmd.account_id)?;

        let mut account = original_account.clone();

        // 2. MUTATION DU MODÈLE RICHE
        // link_external_identity renvoie false si l'ID était déjà identique (idempotence au niveau entité)
        if !account.link_external_identity(&cmd.region_code, cmd.external_id.clone())? {
            return Ok(original_account);
        }
        
        // 3. EXTRACTION DES ÉVÉNEMENTS
        let events = account.pull_events();
        if events.is_empty() {
             return Ok(account);
        }

        // 4. PRÉPARATION DES DONNÉES POUR LA TRANSACTION
        let updated_account = account.clone();
        let repo = Arc::clone(&self.repo);
        let outbox = Arc::clone(&self.outbox);

        // 5. PERSISTANCE TRANSACTIONNELLE ATOMIQUE
        self.tx_manager
            .run_in_transaction(move |mut tx| {
                let repo = Arc::clone(&repo);
                let outbox = Arc::clone(&outbox);
                
                let u_orig = original_account.clone();
                let u_upd = account.clone();
                let evs = events.clone();

                Box::pin(async move {
                    // Sauvegarde avec l'original pour l'Optimistic Lock et la gestion des index
                    repo.save(&u_upd, Some(&u_orig), Some(&mut *tx)).await?;

                    for event in evs {
                        outbox.save(&mut *tx, event.as_ref()).await?;
                    }
                    tx.commit().await?;
                    Ok(())
                })
            })
            .await?;

        Ok(updated_account)
    }
}