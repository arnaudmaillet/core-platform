// crates/account/src/application/shadowban_account/shadowban_account_use_case.rs

use async_trait::async_trait;
use shared_kernel::application::CommandHandler;
use shared_kernel::errors::Result;

use crate::application::context::AccountContext;
use crate::application::use_cases::moderation::shadowban::ShadowbanCommand;

pub struct ShadowbanHandler;

#[async_trait]
impl CommandHandler for ShadowbanHandler {
    type Context = AccountContext;
    type Command = ShadowbanCommand;
    type Output = ();

    async fn handle(&self, ctx: &AccountContext, cmd: ShadowbanCommand) -> Result<Self::Output> {
        let mut account = ctx.account().await?;
        account.shadowban(cmd.reason)?;
        ctx.save(&mut account, Some(cmd.command_id)).await?;

        Ok(())
    }
}