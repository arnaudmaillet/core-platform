// crates/account/src/application/update_settings/mod.rs
use crate::application::context::AccountContext;
use crate::application::use_cases::settings::UpdatePreferencesCommand;
use async_trait::async_trait;
use shared_kernel::application::CommandHandler;
use shared_kernel::core::Result;

pub struct UpdatePreferencesHandler;

#[async_trait]
impl CommandHandler for UpdatePreferencesHandler {
    type Context = AccountContext;
    type Command = UpdatePreferencesCommand;
    type Output = ();

    async fn handle(
        &self,
        ctx: &AccountContext,
        cmd: UpdatePreferencesCommand,
    ) -> Result<Self::Output> {
        let mut account = ctx.account().await?;

        if let Some(privacy) = cmd.privacy {
            account.update_privacy_preferences(privacy)?;
        }

        if let Some(appearance) = cmd.appearance {
            account.update_appearance_preferences(appearance)?;
        }

        if let Some(notification) = cmd.notifications {
            account.update_notifications_preferences(notification)?;
        }

        ctx.save(&mut account, Some(cmd.command_id)).await?;
        Ok(())
    }
}
