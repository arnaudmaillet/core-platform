// crates/profile/src/application/use_cases/identity/create_profile/mod.rs

use crate::application::context::ProfileCommandContext;
use crate::commands::CreateProfileCommand;
use crate::domain::entities::Profile;
use crate::types::DisplayName;
use async_trait::async_trait;
use shared_kernel::command::CommandHandler;
use shared_kernel::core::Result;

pub struct CreateProfileHandler;

#[async_trait]
impl CommandHandler for CreateProfileHandler {
    type Context = ProfileCommandContext;
    type Command = CreateProfileCommand;
    type Output = ();

    async fn handle(
        &self,
        ctx: &ProfileCommandContext,
        cmd: CreateProfileCommand,
    ) -> Result<Self::Output> {
        if !ctx
            .ensure_creatable(cmd.command_id, cmd.region, &cmd.handle)
            .await?
        {
            return Ok(());
        }

        let display_name = DisplayName::from_raw(cmd.handle.as_str());
        let mut profile = Profile::builder(cmd.account_id, cmd.profile_id, cmd.handle)?
            .with_display_name(display_name)
            .build()?;
        profile.create_profile()?;
        ctx.save(&mut profile, Some(cmd.command_id)).await?;

        Ok(())
    }
}
