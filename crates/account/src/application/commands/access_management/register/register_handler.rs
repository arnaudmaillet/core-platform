use async_trait::async_trait;
use shared_kernel::command::CommandHandler;
use shared_kernel::core::{Error, Result, RetryConfig, TransactionManager};
use std::marker::PhantomData;

use crate::application::commands::access_management::RegisterCommand;
use crate::application::context::AccountCommandContext;
use crate::domain::entities::Account;

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
        if !ctx.ensure_creatable(cmd.command_id, cmd.region).await? {
            return Ok(());
        }
        if let Some(ref ext_id) = cmd.sub_id {
            let existing = ctx
                .app()
                .account_repo()
                .find_by_sub_id(ctx.region(), ext_id, None)
                .await?;

            if existing.is_some() {
                return Err(Error::already_exists(
                    "Account",
                    "sub_id",
                    ext_id.to_string(),
                ));
            }
        }

        let account_id = cmd.account_id;
        let mut builder = Account::builder(account_id, cmd.identifier);

        if let Some(ext_id) = cmd.sub_id {
            builder = builder.with_sub_id(ext_id);
        }

        let mut account = builder.with_locale(cmd.locale).build()?;
        account.register(cmd.ip_addr)?;
        ctx.save(&mut account, Some(cmd.command_id)).await?;

        Ok(())
    }

    fn retry_config(&self) -> RetryConfig {
        RetryConfig {
            max_retries: 0,
            initial_backoff_ms: 0,
        }
    }
}
