#[cfg(test)]
mod tests {
    use crate::application::context::AccountContext;
    use crate::application::use_cases::lifecycle::{DeactivateCommand, DeactivateHandler};
    use crate::application::utils::TestFixture;
    use crate::domain::events::AccountEvent;
    use crate::domain::value_objects::AccountState;
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::value_objects::RegionCode;
    use shared_kernel::errors::{DomainError, Result};
    use uuid::Uuid;

    #[tokio::test]
    async fn test_deactivate_account_success() -> Result<()> {
        let f = TestFixture::new();

        // 1. Arrange : Compte initial actif
        let account = f.account_builder()?.build()?;
        let version_snapshot = account.version();
        f.account_repo().insert(account);

        let cmd = DeactivateCommand {
            command_id: Uuid::new_v4(),
            account_id: f.account_id(),
            reason: None,
        };

        // 2. Act
        f.bus()
            .execute::<AccountContext, DeactivateCommand, ()>(f.account_ctx().clone(), cmd)
            .await?;

        // 3. Assert
        f.assert_account(|acc| {
            assert_eq!(*acc.identity().state(), AccountState::DEACTIVATED);
            assert_eq!(acc.version(), version_snapshot + 1);
        })
        .await?;

        f.assert_outbox(1, Some(AccountEvent::DEACTIVATED));

        Ok(())
    }

    #[tokio::test]
    async fn test_deactivate_technical_idempotency() -> Result<()> {
        let f = TestFixture::new();
        let cmd_id = Uuid::new_v4();

        // Arrange : Simulation d'une commande de désactivation déjà enregistrée
        f.idempotency_repo().seed(cmd_id);

        let account = f.account_builder()?.build()?;
        let version_snapshot = account.version();
        f.account_repo().insert(account);

        let cmd = DeactivateCommand {
            command_id: cmd_id,
            account_id: f.account_id(),
            reason: None,
        };

        // 2. Act
        let result = f
            .bus()
            .execute::<AccountContext, DeactivateCommand, ()>(f.account_ctx().clone(), cmd)
            .await;

        // 3. Assert
        assert!(matches!(result, Err(DomainError::AlreadyExists { .. })));

        f.assert_account(|acc| {
            assert_eq!(*acc.identity().state(), AccountState::PENDING);
            assert_eq!(acc.version(), version_snapshot);
        })
        .await?;

        f.assert_outbox(0, None);

        Ok(())
    }

    #[tokio::test]
    async fn test_deactivate_business_idempotency() -> Result<()> {
        let f = TestFixture::new();

        // Arrange : Compte DÉJÀ désactivé
        let mut account = f.account_builder()?.build()?;
        account.deactivate(None)?;

        let version_snapshot = account.version();
        f.account_repo().insert(account);

        let cmd = DeactivateCommand {
            command_id: Uuid::new_v4(),
            account_id: f.account_id(),
            reason: None,
        };

        // 2. Act
        f.bus()
            .execute::<AccountContext, DeactivateCommand, ()>(f.account_ctx().clone(), cmd)
            .await?;

        // 3. Assert
        f.assert_account(|acc| {
            assert_eq!(*acc.identity().state(), AccountState::DEACTIVATED);
            assert_eq!(acc.version(), version_snapshot);
        })
        .await?;

        f.assert_outbox(0, None);

        Ok(())
    }

    #[tokio::test]
    async fn test_deactivate_not_found() -> Result<()> {
        let f = TestFixture::new();

        let cmd = DeactivateCommand {
            command_id: Uuid::new_v4(),
            account_id: f.account_id(),
            reason: None,
        };

        // Act
        let result = f
            .bus()
            .execute::<AccountContext, DeactivateCommand, ()>(f.account_ctx().clone(), cmd)
            .await;

        // Assert
        assert!(matches!(result, Err(DomainError::NotFound { .. })));
        f.assert_outbox(0, None);

        Ok(())
    }

    #[tokio::test]
    async fn test_region_mismatch_returns_not_found() -> Result<()> {
        let f = TestFixture::new();
        let wrong_region = RegionCode::from_raw("US");

        // Arrange : Compte aux USA, mais contexte Europe
        let account = f.account_builder_for(wrong_region)?.build()?;
        let version_snapshot = account.version();
        f.account_repo().insert(account);

        let cmd = DeactivateCommand {
            command_id: Uuid::new_v4(),
            account_id: f.account_id(),
            reason: None,
        };

        // Act
        let result = f
            .bus()
            .execute::<AccountContext, DeactivateCommand, ()>(f.account_ctx().clone(), cmd)
            .await;

        // Assert : Obfuscation de sécurité
        assert!(matches!(result, Err(DomainError::NotFound { .. })));

        let saved = f.account_repo().find_direct(&f.account_id()).unwrap();
        assert_eq!(saved.version(), version_snapshot);
        f.assert_outbox(0, None);

        Ok(())
    }
}
