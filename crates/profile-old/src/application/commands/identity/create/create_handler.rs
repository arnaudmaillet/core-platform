use crate::commands::CreateProfileCommand;
use crate::context::ProfileCommandCtx;
use crate::domain::entities::Profile;
use crate::types::DisplayName;
use async_trait::async_trait;
use shared_kernel::command::CommandHandler;
use shared_kernel::core::{Error, ErrorCode, Result};

pub struct CreateProfileHandler;

impl CreateProfileHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl CommandHandler for CreateProfileHandler {
    type Context = ProfileCommandCtx;
    type Command = CreateProfileCommand;
    type Output = ();

    async fn handle(
        &self,
        ctx: &ProfileCommandCtx,
        cmd: CreateProfileCommand,
    ) -> Result<Self::Output> {
        if cmd.region != ctx.server_region() {
            return Err(Error::validation(
                "region",
                "Routing mismatch: Attempting to create a profile for another region on this cluster",
            ));
        }

        let slug_hash = cmd.handle.to_sha256_hash();
        let display_name = DisplayName::from_raw(cmd.handle.as_str());
        let handle_str = cmd.handle.as_str().to_string();

        let mut profile = Profile::builder(cmd.account_id, cmd.target.id, cmd.handle)?
            .with_display_name(display_name)
            .build()?;

        profile.create_profile()?;

        if let Err(err) = ctx
            .routing_repo()
            .register_routing(cmd.target.id, &slug_hash, cmd.region)
            .await
        {
            if err.code == ErrorCode::ConcurrencyConflict {
                return Err(Error::already_exists("Profile", "handle", handle_str));
            }
            return Err(err);
        }

        if let Err(save_err) = ctx.save(&mut profile, cmd.command_id).await {
            tracing::error!(
                profile_id = %cmd.target.id,
                command_id = %cmd.command_id,
                error = ?save_err,
                "Échec de la sauvegarde du profil. Lancement de la compensation du routage handle..."
            );

            // Compensation : Libération du handle
            let _ = ctx
                .routing_repo()
                .delete_routing(cmd.target.id, &slug_hash)
                .await;

            return Err(save_err);
        }

        Ok(())
    }
}
