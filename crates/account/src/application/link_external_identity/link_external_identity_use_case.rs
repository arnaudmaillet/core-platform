// crates/account/src/application/link_external_identity/link_external_identity_use_case.rs

use crate::application::link_external_identity::LinkExternalIdentityCommand;
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
    account_repo: Arc<dyn AccountRepository>,
    outbox_repo: Arc<dyn OutboxRepository>,
    tx_manager: Arc<dyn TransactionManager>,
}

impl LinkExternalIdentityUseCase {
    pub fn new(
        account_repo: Arc<dyn AccountRepository>,
        outbox_repo: Arc<dyn OutboxRepository>,
        tx_manager: Arc<dyn TransactionManager>,
    ) -> Self {
        Self {
            account_repo,
            outbox_repo,
            tx_manager,
        }
    }

    pub async fn execute(&self, command: LinkExternalIdentityCommand) -> Result<bool> {
        with_retry(RetryConfig::default(), || async {
            self.try_execute_once(&command).await
        })
        .await
    }

    async fn try_execute_once(&self, cmd: &LinkExternalIdentityCommand) -> Result<bool> {
        // 1. VÉRIFICATION D'UNICITÉ ET LECTURE OPTIMISTE (Hors transaction)

        // On vérifie si l'ID externe est déjà utilisé par quelqu'un d'autre
        if let Some(existing_account_id) = self
            .account_repo
            .find_account_id_by_external_id(&cmd.external_id, None)
            .await?
        {
            if existing_account_id != cmd.internal_account_id {
                return Err(DomainError::AlreadyExists {
                    entity: "Account",
                    field: "external_id",
                    value: cmd.external_id.as_str().to_string(),
                });
            }
            return Ok(false);
        }

        let mut account = self
            .account_repo
            .find_account_by_id(&cmd.internal_account_id, None)
            .await?
            .ok_or_not_found(cmd.internal_account_id.clone())?;

        // 2. MUTATION DU MODÈLE RICHE
        let changed = account.link_external_identity(&cmd.region_code, cmd.external_id.clone())?;
        if !changed {
            return Ok(false);
        }
        
        // 3. EXTRACTION DES ÉVÉNEMENTS
        let events = account.pull_events();
        let account_to_save = account.clone();

        // 5. PERSISTANCE TRANSACTIONNELLE ATOMIQUE
        self.tx_manager
            .run_in_transaction(move |mut tx| {
                let repo = self.account_repo.clone();
                let outbox = self.outbox_repo.clone();
                let u = account_to_save.clone();
                let evs = events;

                Box::pin(async move {
                    repo.save(&u, Some(&mut *tx)).await?;
                    for event in evs {
                        outbox.save(&mut *tx, event.as_ref()).await?;
                    }
                    tx.commit().await?;
                    Ok(())
                })
            })
            .await?;

        Ok(true)
    }
}
