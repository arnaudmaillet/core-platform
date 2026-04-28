#[cfg(test)]
mod tests {
    use crate::application::use_cases::settings::remove_push_token::{
        RemovePushTokenCommand, RemovePushTokenHandler,
    };
    use crate::application::utils::TestFixture;
    use crate::domain::events::AccountEvent;
    use crate::domain::value_objects::AccountState;
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::value_objects::{PushToken, RegionCode};
    use shared_kernel::errors::{DomainError, Result};
    use uuid::Uuid;

    #[tokio::test]
    async fn test_remove_push_token_success() -> Result<()> {
        let f = TestFixture::new();
        let token_to_keep = PushToken::try_new("token_keep_456")?;
        let token_to_remove = PushToken::try_new("token_remove_123")?;

        // 1. Arrange : Utilisation de la closure settings pour injecter les tokens
        let account = f
            .account_builder()?
            .with_state(AccountState::Active)
            .settings(|s| s.with_tokens(vec![token_to_remove.clone(), token_to_keep.clone()]))
            .build()?;

        let version_snapshot = account.version();
        f.account_repo().insert(account);

        let cmd = RemovePushTokenCommand {
            command_id: Uuid::new_v4(),
            account_id: f.account_id(),
            token: token_to_remove.clone(),
        };

        // 2. Act
        f.bus()
            .execute(f.account_ctx(), cmd, RemovePushTokenHandler)
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

        // Arrange
        f.idempotency_repo().seed(cmd_id);

        let account = f
            .account_builder()?
            .with_state(AccountState::Active)
            .build()?;
        f.account_repo().insert(account);

        let cmd = RemovePushTokenCommand {
            command_id: cmd_id,
            account_id: f.account_id(),
            token: token.clone(),
        };

        // Act
        let result = f
            .bus()
            .execute(f.account_ctx(), cmd, RemovePushTokenHandler)
            .await;

        // Assert
        assert!(matches!(result, Err(DomainError::AlreadyExists { .. })));
        f.assert_outbox(0, None);

        Ok(())
    }

    #[tokio::test]
    async fn test_remove_push_token_business_idempotency() -> Result<()> {
        let f = TestFixture::new();
        let non_existent_token = PushToken::try_new("valid_but_missing_token")?;

        // 1. Arrange : Compte sans tokens
        let account = f
            .account_builder()?
            .with_state(AccountState::Active)
            .build()?;

        let version_snapshot = account.version();
        f.account_repo().insert(account);

        let cmd = RemovePushTokenCommand {
            command_id: Uuid::new_v4(),
            account_id: f.account_id(),
            token: non_existent_token.clone(),
        };

        // 2. Act
        f.bus()
            .execute(f.account_ctx(), cmd, RemovePushTokenHandler)
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

    #[tokio::test]
    async fn test_region_mismatch_returns_not_found() -> Result<()> {
        let f = TestFixture::new();
        let wrong_region = RegionCode::from_raw("us");
        let token = PushToken::try_new("valid_token_123")?;

        // Arrange : Compte US dans contexte EU
        let account = f
            .account_builder_for(wrong_region)?
            .with_state(AccountState::Active)
            .build()?;

        let version_snapshot = account.version();
        f.account_repo().insert(account);

        let cmd = RemovePushTokenCommand {
            command_id: Uuid::new_v4(),
            account_id: f.account_id(),
            token: token.clone(),
        };

        // Act
        let result = f
            .bus()
            .execute(f.account_ctx(), cmd, RemovePushTokenHandler)
            .await;

        // Assert
        assert!(matches!(result, Err(DomainError::NotFound { .. })));

        // Vérification intégrité
        let saved = f.account_repo().find_direct(&f.account_id()).unwrap();
        assert_eq!(saved.version(), version_snapshot);

        Ok(())
    }
}
