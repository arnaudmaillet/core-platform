// crates/account/src/application/use_cases/verify_email.rs

use async_trait::async_trait;
use std::sync::Arc;

use crate::application::commands::access_management::VerifyEmailCommand;
use crate::application::context::AccountCommandCtx;
use crate::repositories::OtpRepository;
use crate::types::AccountState;
use chrono::Utc;
use shared_kernel::command::CommandHandler;
use shared_kernel::core::{Error, Result, RetryConfig};

pub struct VerifyEmailHandler {
    otp_repository: Arc<dyn OtpRepository>,
}

impl VerifyEmailHandler {
    pub fn new(otp_repository: Arc<dyn OtpRepository>) -> Self {
        Self { otp_repository }
    }
}

#[async_trait]
impl CommandHandler for VerifyEmailHandler {
    type Context = AccountCommandCtx;
    type Command = VerifyEmailCommand;
    type Output = ();

    async fn handle(
        &self,
        ctx: &AccountCommandCtx,
        cmd: VerifyEmailCommand,
    ) -> Result<Self::Output> {
        let account_id = cmd.target.id;
        let stored_code = self.otp_repository.get_code(&account_id, "email").await?;

        let is_valid = match stored_code {
            Some(code) => code == cmd.code,
            None => {
                return Err(Error::validation(
                    "code",
                    "Verification code expired or not requested",
                ));
            }
        };

        if !is_valid {
            return Err(Error::validation("code", "Invalid verification code"));
        }
        let mut account = ctx.fetch_verified(&cmd.target).await?;
        let now = Utc::now();
        let changed = account.verify_email(now)?;

        if changed {
            ctx.save(&mut account, cmd.command_id).await?;

            if account.identity().is_active() {
                ctx.global_registry()
                    .update_state(account_id, AccountState::ACTIVE)
                    .await?;
            }
        }

        let _ = self.otp_repository.invalidate(&account_id, "email").await;

        Ok(())
    }

    fn retry_config(&self) -> RetryConfig {
        RetryConfig {
            max_retries: 0,
            initial_backoff_ms: 0,
        }
    }
}
