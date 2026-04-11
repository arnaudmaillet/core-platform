// crates/account/src/application/link_external_identity/link_external_identity_use_case.rs

use shared_kernel::domain::events::{AggregateRoot, DomainEvent};
use shared_kernel::domain::utils::{RetryConfig, with_retry};
use shared_kernel::errors::{DomainError, Result};

use crate::application::context::AccountContext;
use crate::application::use_cases::access_management::link_external_identity::LinkExternalIdentityCommand;
use crate::domain::account::entities::AccountIdentity;

pub struct LinkExternalIdentityUseCase;

impl LinkExternalIdentityUseCase {
    pub fn new() -> Self {
        Self
    }

    pub async fn execute(
        &self, 
        ctx: &AccountContext, 
        cmd: LinkExternalIdentityCommand
    ) -> Result<AccountIdentity> {
        with_retry(RetryConfig::default(), || async {
            self.try_execute_once(ctx, &cmd).await
        })
        .await
    }

    async fn try_execute_once(
        &self, 
        ctx: &AccountContext, 
        cmd: &LinkExternalIdentityCommand
    ) -> Result<AccountIdentity> {
        // 1. Sécurité : On s'assure que le contexte est bien celui du compte visé
        let _ = ctx.ensure_id(&cmd.account_id);

        // 2. Vérification d'unicité (Hors transaction pour la performance)
        // Est-ce que cet ExternalId est déjà lié à QUELQU'UN d'autre ?
        if let Some(existing_owner_id) = ctx.identity_repo().resolve_id_from_external_id(&cmd.external_id).await? {
            if existing_owner_id != cmd.account_id {
                return Err(DomainError::AlreadyExists {
                    entity: "AccountIdentity",
                    field: "external_id",
                    value: cmd.external_id.to_string(),
                });
            }
        }

        // 3. Récupération de l'état actuel (via le cache du contexte)
        let original_identity = ctx.identity().await?;
        let mut identity = original_identity.clone();

        // 4. Mutation métier
        // .link_external_identity() renvoie false si l'ID est déjà le même (idempotence)
        if !identity.link_external_identity(cmd.external_id.clone())? {
            return Ok(original_identity);
        }

        // 5. Extraction et préparation des événements
        let pulled_events = identity.pull_events();
        if pulled_events.is_empty() {
            return Ok(identity);
        }
        let events: Vec<&dyn DomainEvent> = pulled_events.iter().map(|e| e.as_ref()).collect();

        // 6. Persistance Atomique
        let mut tx = ctx.begin_transaction().await?;

        ctx.save_identity(&identity, Some(&original_identity), &mut *tx).await?;
        ctx.outbox_repo().save_all(&mut *tx, &events).await?;

        tx.commit().await?;

        Ok(identity)
    }
}