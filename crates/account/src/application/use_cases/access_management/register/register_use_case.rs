// crates/account/src/application/use_cases/access_management/register/mod.rs

use shared_kernel::domain::events::{AggregateRoot, DomainEvent};
use shared_kernel::domain::utils::{RetryConfig, with_retry};
use shared_kernel::errors::{DomainError, Result};

use crate::application::context::AccountContext;
use crate::application::use_cases::access_management::register::RegisterCommand;
use crate::domain::account::entities::{AccountIdentity, AccountMetadata, AccountSettings};

pub struct RegisterUseCase;

impl RegisterUseCase {
    pub fn new() -> Self {
        Self
    }

    pub async fn execute(
        &self, 
        ctx: &AccountContext, 
        cmd: RegisterCommand
    ) -> Result<AccountIdentity> {
        with_retry(RetryConfig::default(), || async {
            self.try_execute_once(ctx, &cmd).await
        })
        .await
    }

    async fn try_execute_once(
        &self, 
        ctx: &AccountContext, 
        cmd: &RegisterCommand
    ) -> Result<AccountIdentity> {
        let account_id = *ctx.account_id();

        // Vérification d'unicité (Optimiste)
        if ctx.identity_repo().exists_by_external_id(&cmd.external_id).await? {
            return Err(DomainError::AlreadyExists {
                entity: "AccountIdentity",
                field: "external_id",
                value: cmd.external_id.to_string(),
            });
        }

        if ctx.identity_repo().exists_by_email(&cmd.email).await? {
            return Err(DomainError::AlreadyExists {
                entity: "AccountIdentity",
                field: "email",
                value: cmd.email.to_string(),
            });
        }

        // 1. INITIALISATION DES 3 AGRÉGATS
        let mut identity = AccountIdentity::builder(
            account_id,
            cmd.region.clone(),
            cmd.email.clone(),
            cmd.external_id.clone(),
        )
        .with_locale(cmd.locale.clone())
        .build();

        let metadata = AccountMetadata::builder(account_id)
            .with_ip_addr(cmd.ip_addr.clone())
            .build();

        let settings = AccountSettings::builder(account_id).build();

        // 2. LOGIQUE MÉTIER & ÉVÉNEMENTS
        if !identity.register(cmd.region.clone(), cmd.ip_addr.clone())? {
            return Err(DomainError::Unexpected("Registration logic failed".into()));
        }

        let pulled_events = identity.pull_events();
        let events: Vec<&dyn DomainEvent> = pulled_events.iter().map(|e| e.as_ref()).collect();

        // 3. PERSISTANCE TRANSACTIONNELLE
        // On ouvre une transaction sur le shard de la région demandée
        let mut tx = ctx.begin_transaction().await?;

        // On sauvegarde les 3 piliers du compte
        // On passe None pour "original" car c'est une création (INSERT)
        ctx.save_identity(&identity, None, &mut *tx).await?;
        ctx.save_metadata(&metadata, None, &mut *tx).await?;
        ctx.save_settings(&settings, None, &mut *tx).await?;

        // On persiste les événements
        ctx.outbox_repo().save_all(&mut *tx, &events).await?;

        tx.commit().await?;

        Ok(identity)
    }
}