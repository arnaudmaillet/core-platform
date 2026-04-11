// crates/account/src/application/update_locale/update_locale_use_case.rs

use shared_kernel::domain::events::{AggregateRoot, DomainEvent};
use shared_kernel::domain::repositories::OutboxRepository;
use shared_kernel::domain::utils::{RetryConfig, with_retry};
use shared_kernel::errors::Result;

use crate::application::context::AccountContext;
use crate::application::use_cases::settings::update_locale::UpdateLocaleCommand;
use crate::domain::account::entities::AccountIdentity;

pub struct UpdateLocaleUseCase;

impl UpdateLocaleUseCase {
    pub fn new() -> Self {
        Self
    }

    pub async fn execute(&self, ctx: &AccountContext, cmd: UpdateLocaleCommand) -> Result<AccountIdentity> {
        with_retry(RetryConfig::default(), || async {
            self.try_execute_once(ctx, &cmd).await
        })
        .await
    }

    async fn try_execute_once(&self, ctx: &AccountContext, cmd: &UpdateLocaleCommand) -> Result<AccountIdentity> {
        let _ = ctx.ensure_id(&cmd.account_id);

        let original_identity = ctx.identity().await?;
        let mut identity = original_identity.clone();

        if !identity.update_locale(cmd.locale.clone())? {
            return Ok(original_identity);
        }

        let pulled_events = identity.pull_events();
        if pulled_events.is_empty() {
            return Ok(identity);
        }

        let events: Vec<&dyn DomainEvent> = pulled_events.iter().map(|e| e.as_ref()).collect();
        let mut tx = ctx.begin_transaction().await?;

        ctx.save_identity(&identity, Some(&original_identity), &mut *tx).await?;
        ctx.outbox_repo().save_all(&mut *tx, &events).await?;
        tx.commit().await?;

        Ok(identity)
    }
}
