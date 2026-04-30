// crates/account/src/application/link_external_identity/link_external_identity_handler.rs

use crate::application::{
    context::AccountContext,
    use_cases::access_management::link_external_identity::LinkExternalIdentityCommand,
};
use async_trait::async_trait;
use shared_kernel::{application::CommandHandler, domain::utils::RetryConfig, errors::Result};

pub struct LinkExternalIdentityHandler;

#[async_trait]
impl CommandHandler for LinkExternalIdentityHandler {
    type Context = AccountContext;
    type Command = LinkExternalIdentityCommand;
    type Output = ();

    async fn handle(
        &self,
        ctx: &AccountContext,
        cmd: LinkExternalIdentityCommand,
    ) -> Result<Self::Output> {
        let mut account = ctx.account().await?;
        account.link_external_identity(cmd.external_id)?;
        ctx.save(&mut account, Some(cmd.command_id)).await?;

        Ok(())
    }
    
    fn retry_config(&self) -> RetryConfig {
        RetryConfig {
            max_retries: 5,
            initial_backoff_ms: 50,
        }
    }
}
