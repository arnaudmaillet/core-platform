// crates/account/src/application/ban_account/ban_account_use_case.rs

use async_trait::async_trait;
use shared_kernel::application::CommandHandler;
use shared_kernel::core::Result;

use crate::application::context::AccountContext;
use crate::application::use_cases::moderation::BanCommand;

pub struct BanHandler;

#[async_trait]
impl CommandHandler for BanHandler {
    type Context = AccountContext;
    type Command = BanCommand;
    type Output = ();

    async fn handle(&self, ctx: &AccountContext, cmd: BanCommand) -> Result<Self::Output> {
        let mut account = ctx.account().await?;
        
        account.ban(cmd.reason)?;
        ctx.save(&mut account, Some(cmd.command_id)).await?;

        Ok(())
    }
}