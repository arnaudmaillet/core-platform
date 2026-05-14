// crates/account/src/application/use_cases/access_management/register/mod.rs
use async_trait::async_trait;

use shared_kernel::command::CommandHandler;
use shared_kernel::core::{Error, Result, RetryConfig};

use crate::application::commands::access_management::RegisterCommand;
use crate::application::context::AccountContext;
use crate::domain::entities::Account;

pub struct RegisterHandler;

#[async_trait]
impl CommandHandler for RegisterHandler {
    type Context = AccountContext;
    type Command = RegisterCommand;
    type Output = ();

    async fn handle(&self, ctx: &AccountContext, cmd: RegisterCommand) -> Result<Self::Output> {
        ctx.check_idempotency(cmd.command_id).await?;
        // 1. VÉRIFICATION D'UNICITÉ
        if let Some(ref ext_id) = cmd.sub_id {
            let existing = ctx
                .app_ctx()
                .account_repo()
                .find_by_sub_id(ext_id, None)
                .await?;

            if existing.is_some() {
                return Err(Error::already_exists(
                    "Account",
                    "sub_id",
                    ext_id.to_string(),
                ));
            }
        }

        // 2. Construction de l'agrégat
        let account_id = ctx.account_id().clone();
        let mut builder = Account::builder(account_id, cmd.identifier);

        if let Some(ext_id) = cmd.sub_id {
            builder = builder.with_sub_id(ext_id);
        }

        let mut account = builder.with_locale(cmd.locale).build()?;

        // 3. Logique métier
        account.register(cmd.region.clone(), cmd.ip_addr)?;

        // 4. Persistance (atomique avec Outbox et Idempotence)
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
