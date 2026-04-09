// crates/account/src/application/add_push_token/add_push_token_use_case.rs

use crate::application::context::AccountContext;
use crate::application::use_cases::settings::add_push_token::AddPushTokenCommand;
use crate::domain::account::entities::AccountSettings;
use shared_kernel::domain::events::{AggregateRoot, DomainEvent};
use shared_kernel::domain::utils::{RetryConfig, with_retry};
use shared_kernel::errors::Result;

pub struct AddPushTokenUseCase;

impl AddPushTokenUseCase {
    pub fn new() -> Self {
        Self
    }

    pub async fn execute(&self, ctx: &AccountContext, cmd: AddPushTokenCommand) -> Result<AccountSettings> {
        // En Hyperscale, les conflits de tokens sont rares mais possibles si
        // l'utilisateur se connecte sur deux devices en même temps.
        with_retry(RetryConfig::default(), || async {
            self.try_execute_once(ctx, &cmd).await
        })
        .await
    }

    async fn try_execute_once(&self, ctx: &AccountContext, cmd: &AddPushTokenCommand) -> Result<AccountSettings> {
        ctx.ensure_id(&cmd.account_id);

        let original_settings = ctx.settings().await?;
        let mut settings = original_settings.clone();

        if !settings.add_push_token(cmd.token.clone())? {
            return Ok(original_settings);
        };

        let pulled_events = settings.pull_events();
        if pulled_events.is_empty() {
            return Ok(settings);
        }

        let events: Vec<&dyn DomainEvent> = pulled_events.iter().map(|e| e.as_ref()).collect();
        let mut tx = ctx.begin_transaction().await?;

        ctx.save_settings(&settings, Some(&original_settings), &mut *tx).await?;
        ctx.outbox_repo().save_all(&mut *tx, &events).await?;
        tx.commit().await?;


        Ok(settings)
    }
}
