// crates/account/src/application/link_external_identity/link_external_identity_use_case.rs

use std::sync::Arc;
use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::domain::entities::EntityOptionExt;
use shared_kernel::domain::repositories::OutboxRepository;
use shared_kernel::domain::transaction::TransactionManager;
use shared_kernel::errors::{DomainError, Result};
use shared_kernel::infrastructure::{with_retry, RetryConfig, TransactionManagerExt};
use crate::application::link_external_identity::LinkExternalIdentityCommand;
use crate::domain::repositories::AccountRepository;

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

    pub async fn execute(&self, command: LinkExternalIdentityCommand) -> Result<()> {
        with_retry(RetryConfig::default(), || async {
            self.try_execute_once(&command).await
        }).await
    }

    async fn try_execute_once(&self, cmd: &LinkExternalIdentityCommand) -> Result<()> {
        // 1. VÉRIFICATION D'UNICITÉ ET LECTURE OPTIMISTE (Hors transaction)

        // On vérifie si l'ID externe est déjà utilisé par quelqu'un d'autre
        if let Some(existing_account_id) = self.account_repo
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
            return Ok(());
        }

        let mut account = self.account_repo
            .find_account_by_id(&cmd.internal_account_id, None)
            .await?
            .ok_or_not_found(cmd.internal_account_id.clone())?;

        // 2. MUTATION DU MODÈLE RICHE
        account.link_external_identity(cmd.external_id.clone())?;

        // 3. EXTRACTION DES ÉVÉNEMENTS
        let events = account.pull_events();

        // 4. IDEMPOTENCE APPLICATIVE
        if events.is_empty() {
            return Ok(());
        }

        let account_to_save = account.clone();

        // 5. PERSISTANCE TRANSACTIONNELLE ATOMIQUE
        self.tx_manager.run_in_transaction(move |mut tx| {
            let repo = self.account_repo.clone();
            let outbox = self.outbox_repo.clone();
            let u = account_to_save.clone();
            let evs = events;

            Box::pin(async move {
                repo.save(&u, Some(&mut *tx)).await?;
                for event in evs {
                    outbox.save(event.as_ref(), Some(&mut *tx)).await?;
                }

                Ok(())
            })
        }).await?;

        Ok(())
    }
}