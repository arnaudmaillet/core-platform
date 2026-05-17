#[cfg(test)]
mod tests {
    use crate::application::commands::settings::RemovePushTokenCommand;
    use crate::application::context::AccountContext;
    use crate::application::utils::TestFixture;
    use crate::domain::events::AccountEvent;
    use crate::domain::types::AccountState;
    use shared_kernel::{
        command::CommandTarget,
        core::{Error, ErrorCode, Result, Versioned},
        security::PushToken,
    };
    use uuid::Uuid;

    #[tokio::test]
    async fn test_remove_push_token_success() -> Result<()> {
        let f = TestFixture::new();
        let token_to_keep = PushToken::try_new("token_keep_456")?;
        let token_to_remove = PushToken::try_new("token_remove_123")?;

        // 1. Arrange : Utilisation de la closure settings pour injecter les tokens
        let account = f
            .account_builder()?
            .with_state(AccountState::ACTIVE)
            .settings(|s| s.with_tokens(vec![token_to_remove.clone(), token_to_keep.clone()]))
            .build()?;

        let version_snapshot = account.version();
        f.account_repo().insert(account);

        let cmd = RemovePushTokenCommand {
            command_id: Uuid::new_v4(),
            target: CommandTarget::new(f.account_id().clone(), f.region(), version_snapshot),
            token: token_to_remove.clone(),
        };

        // 2. Act
        f.bus()
            .execute::<AccountContext, RemovePushTokenCommand, ()>(f.account_ctx().clone(), cmd)
            .await?;

        // 3. Assert
        f.assert_account(|acc| {
            let tokens = acc.settings().push_tokens();
            assert!(
                !tokens.contains(&token_to_remove),
                "Le token doit être supprimé"
            );
            assert!(
                tokens.contains(&token_to_keep),
                "Le token à garder doit rester"
            );
            assert_eq!(acc.version(), version_snapshot + 1);
        })
        .await?;

        f.assert_outbox(1, Some(AccountEvent::PUSH_TOKEN_REMOVED));

        Ok(())
    }

    #[tokio::test]
    async fn test_remove_push_token_technical_idempotency() -> Result<()> {
        let f = TestFixture::new();
        let cmd_id = Uuid::new_v4();
        let token = PushToken::try_new("token_test_123")?;

        f.idempotency_repo().seed(cmd_id);

        let account = f
            .account_builder()?
            .with_state(AccountState::ACTIVE)
            .build()?;
        let version_snapshot = account.version();
        f.account_repo().insert(account);

        let cmd = RemovePushTokenCommand {
            command_id: cmd_id,
            target: CommandTarget::new(f.account_id().clone(), f.region(), version_snapshot),
            token: token.clone(),
        };

        let result = f
            .bus()
            .execute::<AccountContext, RemovePushTokenCommand, ()>(f.account_ctx().clone(), cmd)
            .await;

        assert!(
            result.is_ok(),
            "L'idempotence technique doit être transparente (Ok)"
        );

        f.assert_outbox(0, None);

        f.assert_account(|acc| {
            assert_eq!(
                acc.version(),
                version_snapshot,
                "La version ne doit pas avoir augmenté"
            );
        })
        .await?;

        Ok(())
    }

    #[tokio::test]
    async fn test_remove_push_token_business_idempotency() -> Result<()> {
        let f = TestFixture::new();
        let non_existent_token = PushToken::try_new("valid_but_missing_token")?;

        // 1. Arrange : Compte sans tokens
        let account = f
            .account_builder()?
            .with_state(AccountState::ACTIVE)
            .build()?;

        let version_snapshot = account.version();
        f.account_repo().insert(account);

        let cmd = RemovePushTokenCommand {
            command_id: Uuid::new_v4(),
            target: CommandTarget::new(f.account_id().clone(), f.region(), version_snapshot),
            token: non_existent_token.clone(),
        };

        // 2. Act
        f.bus()
            .execute::<AccountContext, RemovePushTokenCommand, ()>(f.account_ctx().clone(), cmd)
            .await?;

        // 3. Assert
        f.assert_account(|acc| {
            assert_eq!(
                acc.version(),
                version_snapshot,
                "La version ne doit pas bouger"
            );
            assert!(acc.settings().push_tokens().is_empty());
        })
        .await?;

        f.assert_outbox(0, None);

        Ok(())
    }
}
