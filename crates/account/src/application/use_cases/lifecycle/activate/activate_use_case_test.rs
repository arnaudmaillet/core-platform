// crates/account/src/application/use_cases/lifecycle/activate/activate_use_case_test.rs

#[cfg(test)]
mod tests {
    use crate::application::context::AccountContext;
    use crate::application::use_cases::lifecycle::{ActivateCommand, ActivateHandler};
    use crate::application::utils::TestFixture;
    use crate::domain::events::AccountEvent;
    use crate::domain::value_objects::AccountState;
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::value_objects::{AuditReason, RegionCode};
    use shared_kernel::errors::{DomainError, Result};
    use uuid::Uuid;

    #[tokio::test]
    async fn test_activate_account_success() -> Result<()> {
        let f = TestFixture::new();

        // 1. Arrange : On crée un compte désactivé
        let mut account = f.account_builder()?.build()?;

        account.deactivate(None)?;
        account.pull_events();

        let version_snapshot = account.version();

        f.account_repo().insert(account);

        let cmd = ActivateCommand {
            command_id: Uuid::new_v4(),
            account_id: f.account_id(),
        };

        f.bus()
            .execute::<AccountContext, ActivateCommand, ()>(f.account_ctx().clone(), cmd)
            .await?;

        f.assert_account(|acc| {
            assert_eq!(*acc.identity().state(), AccountState::Active);
            assert_eq!(acc.version(), version_snapshot + 1);
        })
        .await?;

        // 5. Outbox
        f.assert_outbox(1, Some(AccountEvent::ACTIVATED));

        Ok(())
    }

    #[tokio::test]
    async fn test_activate_technical_idempotency() -> Result<()> {
        let f = TestFixture::new();
        let cmd_id = Uuid::new_v4();

        // Arrange : On simule une commande déjà traitée
        f.idempotency_repo().seed(cmd_id);

        let mut account = f.account_builder()?.build()?;
        account.deactivate(None)?;

        let version_snapshot = account.version();
        f.account_repo().insert(account);

        let cmd = ActivateCommand {
            command_id: cmd_id,
            account_id: f.account_id(),
        };

        // 2. Act
        let result = f
            .bus()
            .execute::<AccountContext, ActivateCommand, ()>(f.account_ctx().clone(), cmd)
            .await;

        // 3. Assert
        assert!(matches!(result, Err(DomainError::AlreadyExists { .. })));

        f.assert_account(|acc| {
            assert_eq!(*acc.identity().state(), AccountState::Deactivated);
            assert_eq!(acc.version(), version_snapshot);
        })
        .await?;

        f.assert_outbox(0, None);

        Ok(())
    }

    #[tokio::test]
    async fn test_activate_business_idempotency() -> Result<()> {
        let f = TestFixture::new();

        // Arrange : Compte déjà actif
        let mut account = f.account_builder()?.build()?;
        account.activate()?;

        let version_snapshot = account.version();
        f.account_repo().insert(account);

        let cmd = ActivateCommand {
            command_id: Uuid::new_v4(),
            account_id: f.account_id(),
        };

        // 2. Act
        f.bus()
            .execute::<AccountContext, ActivateCommand, ()>(f.account_ctx().clone(), cmd)
            .await?;

        // 3. Assert
        f.assert_account(|acc| {
            assert_eq!(*acc.identity().state(), AccountState::Active);
            assert_eq!(acc.version(), version_snapshot);
        })
        .await?;

        f.assert_outbox(0, None);

        Ok(())
    }

    #[tokio::test]
    async fn test_activate_forbidden_if_banned() -> Result<()> {
        let f = TestFixture::new();

        // Arrange
        let mut account = f.account_builder()?.build()?;
        account.ban(AuditReason::try_new("Violation")?)?;
        let version_snapshot = account.version();
        f.account_repo().insert(account);

        let cmd = ActivateCommand {
            command_id: Uuid::new_v4(),
            account_id: f.account_id(),
        };

        // 2. Act
        let result = f
            .bus()
            .execute::<AccountContext, ActivateCommand, ()>(f.account_ctx().clone(), cmd)
            .await;

        // 3. Assert
        assert!(matches!(result, Err(DomainError::Forbidden { .. })));

        f.assert_account(|acc| {
            assert_eq!(*acc.identity().state(), AccountState::Banned);
            assert_eq!(acc.version(), version_snapshot);
        })
        .await?;

        f.assert_outbox(0, None);

        Ok(())
    }

    #[tokio::test]
    async fn test_region_mismatch_returns_not_found() -> Result<()> {
        let f = TestFixture::new();
        let wrong_region = RegionCode::try_new("us")?;

        // Arrange : On insère un compte qui n'est pas dans la région du contexte (eu)
        let account = f.account_builder_for(wrong_region)?.build()?;
        let version_snapshot = account.version();
        f.account_repo().insert(account);

        let cmd = ActivateCommand {
            command_id: Uuid::new_v4(),
            account_id: f.account_id(),
        };

        // 2. Act
        let result = f
            .bus()
            .execute::<AccountContext, ActivateCommand, ()>(f.account_ctx().clone(), cmd)
            .await;

        // 3. Assert
        assert!(matches!(result, Err(DomainError::NotFound { .. })));

        // Vérification directe via le repo (car le contexte ne le trouvera pas)
        let saved = f.account_repo().find_direct(&f.account_id()).unwrap();
        assert_eq!(saved.version(), version_snapshot);
        f.assert_outbox(0, None);

        Ok(())
    }
}
