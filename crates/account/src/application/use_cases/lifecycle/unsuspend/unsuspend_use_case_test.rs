#[cfg(test)]
mod tests {
    use crate::application::use_cases::lifecycle::unsuspend::{UnsuspendCommand, UnsuspendHandler};
    use crate::application::utils::TestFixture;
    use crate::domain::events::AccountEvent;
    use crate::domain::value_objects::AccountState;
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::value_objects::{AuditReason, RegionCode};
    use shared_kernel::errors::{DomainError, Result};
    use uuid::Uuid;

    #[tokio::test]
    async fn test_unsuspend_account_success() -> Result<()> {
        let f = TestFixture::new();

        // 1. Arrange : On crée un compte et on le suspend
        let mut account = f.account_builder()?.build()?;
        account.suspend(AuditReason::try_new("Suspicious activity")?)?;
        let version_snapshot = account.version();
        f.account_repo().insert(account);

        let cmd = UnsuspendCommand {
            command_id: Uuid::new_v4(),
            account_id: f.account_id(),
            reason: AuditReason::try_new("Good behavior")?,
        };

        // 2. Act
        f.bus()
            .execute(f.account_ctx(), cmd, UnsuspendHandler)
            .await?;

        // 3. Assert
        f.assert_account(|acc| {
            assert_eq!(*acc.identity().state(), AccountState::Active);
            assert_eq!(acc.version(), version_snapshot + 1);
        })
        .await?;

        f.assert_outbox(1, Some(AccountEvent::UNSUSPENDED));

        Ok(())
    }

    #[tokio::test]
    async fn test_unsuspend_technical_idempotency() -> Result<()> {
        let f = TestFixture::new();
        let cmd_id = Uuid::new_v4();

        // Arrange : Commande déjà connue de l'infrastructure
        f.idempotency_repo().seed(cmd_id);

        let mut account = f.account_builder()?.build()?;

        account.suspend(AuditReason::try_new("Suspicious activity")?)?;
        let version_snapshot = account.version();
        f.account_repo().insert(account);

        let cmd = UnsuspendCommand {
            command_id: cmd_id,
            account_id: f.account_id(),
            reason: AuditReason::try_new("Duplicate call")?,
        };

        // 2. Act
        let result = f
            .bus()
            .execute(f.account_ctx(), cmd, UnsuspendHandler)
            .await;

        // 3. Assert
        assert!(matches!(result, Err(DomainError::AlreadyExists { .. })));

        f.assert_account(|acc| {
            assert_eq!(*acc.identity().state(), AccountState::Suspended);
            assert_eq!(acc.version(), version_snapshot);
        })
        .await?;

        f.assert_outbox(0, None);

        Ok(())
    }

    #[tokio::test]
    async fn test_unsuspend_business_idempotency() -> Result<()> {
        let f = TestFixture::new();

        // Arrange : Le compte est déjà Actif
        let account = f.account_builder()?.build()?;
        let version_snapshot = account.version();
        f.account_repo().insert(account);

        let cmd = UnsuspendCommand {
            command_id: Uuid::new_v4(),
            account_id: f.account_id(),
            reason: AuditReason::try_new("Already good")?,
        };

        // 2. Act
        f.bus()
            .execute(f.account_ctx(), cmd, UnsuspendHandler)
            .await?;

        // 3. Assert
        f.assert_account(|acc| {
            assert_eq!(*acc.identity().state(), AccountState::Pending);
            assert_eq!(acc.version(), version_snapshot);
        })
        .await?;

        f.assert_outbox(0, None);

        Ok(())
    }

    #[tokio::test]
    async fn test_region_mismatch_returns_not_found() -> Result<()> {
        let f = TestFixture::new();
        let wrong_region = RegionCode::from_raw("us");

        let account = f.account_builder_for(wrong_region)?.build()?;
        let version_snapshot = account.version();
        f.account_repo().insert(account);

        let cmd = UnsuspendCommand {
            command_id: Uuid::new_v4(),
            account_id: f.account_id(),
            reason: AuditReason::try_new("Good behavior")?,
        };

        // Act
        let result = f
            .bus()
            .execute(f.account_ctx(), cmd, UnsuspendHandler)
            .await;

        // Assert
        assert!(matches!(result, Err(DomainError::NotFound { .. })));

        let saved = f.account_repo().find_direct(&f.account_id()).unwrap();
        assert_eq!(saved.version(), version_snapshot);
        f.assert_outbox(0, None);

        Ok(())
    }
}
