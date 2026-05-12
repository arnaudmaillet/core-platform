// crates/account/src/application/use_cases/access_management/register/mod.rs
use async_trait::async_trait;

use shared_kernel::application::CommandHandler;
use shared_kernel::domain::utils::RetryConfig;
use shared_kernel::domain::value_objects::AccountId;
use shared_kernel::core::Result;

use crate::application::context::AccountContext;
use crate::application::use_cases::access_management::RegisterCommand;
use crate::domain::account::entities::Account;

pub struct RegisterHandler;

#[async_trait]
impl CommandHandler for RegisterHandler {
    type Context = AccountContext;
    type Command = RegisterCommand;
    type Output = AccountId;

    async fn handle(&self, ctx: &AccountContext, cmd: RegisterCommand) -> Result<Self::Output> {
        let account_id = cmd.account_id.clone();
        let mut builder = Account::builder(account_id.clone(), cmd.identifier);

        if let Some(ext_id) = cmd.sub_id {
            builder = builder.with_sub_id(ext_id);
        }

        let mut account = builder.with_locale(cmd.locale).build()?;
        account.register(cmd.region.clone(), cmd.ip_addr)?;

        ctx.save(&mut account, Some(cmd.command_id)).await?;

        Ok(account_id)
    }

    fn retry_config(&self) -> RetryConfig {
        RetryConfig {
            max_retries: 0,
            initial_backoff_ms: 0,
        }
    }
}
