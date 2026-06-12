// crates/account/src/application/handlers/register_handler.rs

use async_trait::async_trait;
use chrono::Utc;
use shared_kernel::command::CommandHandler;
use shared_kernel::core::Result;

use crate::application::commands::access_management::RegisterCommand;
use crate::application::context::AccountCommandCtx;
use crate::domain::entities::Account;
use crate::repositories::GlobalIdentityRegistration;
use crate::types::AccountState;

pub struct RegisterHandler;

impl RegisterHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl CommandHandler for RegisterHandler {
    type Context = AccountCommandCtx;
    type Command = RegisterCommand;
    type Output = ();

    async fn handle(
        &self,
        ctx: &AccountCommandCtx,
        cmd: RegisterCommand,
    ) -> Result<Self::Output> {
        let account_id = cmd.target.id;
        let now = Utc::now();

        let registration = GlobalIdentityRegistration {
            account_id,
            region: cmd.region,
            sub_id: cmd.sub_id.clone(),
            identifiers: cmd.identifier.clone(),
            state: AccountState::PENDING,
            created_at: now,
            updated_at: now,
        };

        ctx.global_registry().reserve(&registration).await?;

        let mut builder = Account::builder(account_id, cmd.identifier);
        if let Some(ext_id) = cmd.sub_id {
            builder = builder.with_sub_id(ext_id);
        }

        let mut account = builder.with_locale(cmd.locale).build()?;
        account.register(cmd.ip_addr)?;

        if let Err(e) = ctx.save(&mut account, cmd.command_id).await {
            tracing::error!(
                account_id = %account_id,
                error = %e,
                "Regional account persistence failed after global reservation"
            );
            return Err(e);
        }

        ctx.global_registry()
            .update_state(account_id, AccountState::UNVERIFIED)
            .await?;

        Ok(())
    }
}
