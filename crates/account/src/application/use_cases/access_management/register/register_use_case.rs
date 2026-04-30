// crates/account/src/application/use_cases/access_management/register/mod.rs
use async_trait::async_trait;

use shared_kernel::application::CommandHandler;
use shared_kernel::domain::utils::RetryConfig;
use shared_kernel::domain::value_objects::AccountId;
use shared_kernel::errors::{DomainError, Result};

use crate::application::context::{AccountAppContext, AccountContext};
use crate::application::use_cases::access_management::register::RegisterCommand;
use crate::domain::account::entities::Account;

pub struct RegisterHandler;

#[async_trait]
impl CommandHandler for RegisterHandler {
    type Context = AccountContext;
    type Command = RegisterCommand;
    type Output = AccountId;

    async fn handle(&self, ctx: &AccountContext, cmd: RegisterCommand) -> Result<Self::Output> {
        if let Some(ref ext_id) = cmd.external_id {
            if ctx
                .app_ctx()
                .account_repo()
                .exists_by_external_id(ext_id, None)
                .await?
            {
                return Err(DomainError::AlreadyExists {
                    entity: "Account",
                    field: "external_id",
                    value: ext_id.to_string(),
                });
            }
        }

        let account_id = cmd.account_id.clone();
        let mut builder = Account::builder(account_id.clone(), cmd.region.clone(), cmd.identifier);

        if let Some(ext_id) = cmd.external_id {
            builder = builder.with_external_id(ext_id);
        }

        let mut account = builder.with_locale(cmd.locale).build()?;
        account.register(cmd.region.clone(), cmd.ip_addr)?;

        ctx.save(&mut account, Some(cmd.command_id)).await?;

        Ok(account_id)
    }

    fn retry_config(&self) -> RetryConfig {
        RetryConfig {
            max_retries: 5,
            initial_backoff_ms: 50,
        }
    }
}
