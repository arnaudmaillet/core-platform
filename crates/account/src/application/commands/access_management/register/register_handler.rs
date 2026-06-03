// crates/account/src/application/handlers/register_handler.rs

use async_trait::async_trait;
use chrono::Utc;
use shared_kernel::command::CommandHandler;
use shared_kernel::core::{Result, RetryConfig, TransactionManager};
use std::marker::PhantomData;

use crate::application::commands::access_management::RegisterCommand;
use crate::application::context::AccountCommandContext;
use crate::domain::entities::Account;
use crate::repositories::GlobalIdentityRegistration;
use crate::types::AccountState;

pub struct RegisterHandler<TM> {
    _marker: PhantomData<TM>,
}

impl<TM> RegisterHandler<TM> {
    pub fn new() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

#[async_trait]
impl<TM: TransactionManager + Clone + 'static> CommandHandler for RegisterHandler<TM> {
    type Context = AccountCommandContext<TM>;
    type Command = RegisterCommand;
    type Output = ();

    async fn handle(
        &self,
        ctx: &AccountCommandContext<TM>,
        cmd: RegisterCommand,
    ) -> Result<Self::Output> {
        if !ctx
            .ensure_creatable(cmd.command_id, cmd.target.region)
            .await?
        {
            return Ok(());
        }

        let account_id = cmd.target.id;
        let now = Utc::now();
        let registration = GlobalIdentityRegistration {
            account_id,
            region: ctx.region(),
            sub_id: cmd.sub_id.clone(),
            identifiers: cmd.identifier.clone(),
            state: AccountState::PENDING,
            created_at: now,
            updated_at: now,
        };

        ctx.global_registry().reserve(&registration).await?;

        let mut builder = Account::builder(account_id, cmd.identifier);
        if let Some(ext_id) = cmd.sub_id {
            builder = builder.with_sub_id(ext_id);
        }

        let mut account = builder.with_locale(cmd.locale).build()?;
        account.register(cmd.ip_addr)?;

        if let Err(e) = ctx.save(&mut account, Some(cmd.command_id)).await {
            tracing::error!(
                account_id = %account_id,
                error = %e,
                "Regional account persistence failed after global reservation"
            );
            // On laisse la ligne globale en PENDING. Le Garbage Collector mondial s'occupera
            // de purger la réservation s'il ne voit pas le compte sur le shard régional d'ici 15m.
            return Err(e);
        }

        ctx.global_registry()
            .update_state(account_id, AccountState::UNVERIFIED)
            .await?;

        Ok(())
    }

    fn retry_config(&self) -> RetryConfig {
        RetryConfig {
            max_retries: 0,
            initial_backoff_ms: 0,
        }
    }
}
