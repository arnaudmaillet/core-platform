// crates/account/src/application/update_settings/mod.rs

use crate::application::commands::settings::UpdatePreferencesCommand;
use crate::application::context::AccountContext;
use async_trait::async_trait;
use shared_kernel::command::CommandHandler;
use shared_kernel::core::Result;
use tracing::info;

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
        if !ctx
            .ensure_executable(cmd.command_id, cmd.target.id.region())
            .await?
        {
            return Ok(());
        }

        let mut account = ctx.fetch_verified(&cmd.target).await?;
        let mut changed = false;
        
        if let Some(privacy) = cmd.privacy {
            if account.update_privacy_preferences(privacy)? {
                changed = true;
            }
        }

        if let Some(appearance) = cmd.appearance {
            if account.update_appearance_preferences(appearance)? {
                changed = true;
            }
        }

        if let Some(notification) = cmd.notifications {
            if account.update_notifications_preferences(notification)? {
                changed = true;
            }
        }

        let account_id_str = account.account_id().to_string();
        if changed {
            ctx.save(&mut account, Some(cmd.command_id)).await?;
            info!(
                account_id = %account_id_str,
                command_id = %cmd.command_id,
                "account preferences updated successfully"
            );
        } else {
            info!(
                account_id = %account_id_str,
                "no changes detected in preferences, skipping save"
            );
        }

        Ok(())
    }
}
