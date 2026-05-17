#[cfg(test)]
mod tests {
    use crate::application::commands::lifecycle::UnsuspendCommand;
    use crate::application::context::AccountContext;
    use crate::application::utils::TestFixture;
    use crate::domain::events::AccountEvent;
    use crate::domain::types::AccountState;
    use shared_kernel::command::CommandTarget;
    use shared_kernel::core::{ErrorCode, Result, Versioned};
    use shared_kernel::types::AuditReason;
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
            target: CommandTarget::new(f.account_id(), f.region(), version_snapshot),
            reason: AuditReason::try_new("Good behavior")?,
        };

        // 2. Act
        f.bus()
            .execute::<AccountContext, UnsuspendCommand, ()>(f.account_ctx().clone(), cmd)
            .await?;

        // 3. Assert
        f.assert_account(|acc| {
            assert_eq!(*acc.identity().state(), AccountState::ACTIVE);
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
            target: CommandTarget::new(f.account_id(), f.region(), version_snapshot),
            reason: AuditReason::try_new("Duplicate call")?,
        };

        // 2. Act
        let result = f
            .bus()
            .execute::<AccountContext, UnsuspendCommand, ()>(f.account_ctx().clone(), cmd)
            .await;

        // 3. Assert
        assert!(
            result.is_ok(),
            "L'idempotence technique doit être transparente (Ok)"
        );

        f.assert_account(|acc| {
            assert_eq!(*acc.identity().state(), AccountState::SUSPENDED);
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
            target: CommandTarget::new(f.account_id(), f.region(), version_snapshot),
            reason: AuditReason::try_new("Already good")?,
        };

        // 2. Act
        f.bus()
            .execute::<AccountContext, UnsuspendCommand, ()>(f.account_ctx().clone(), cmd)
            .await?;

        // 3. Assert
        f.assert_account(|acc| {
            assert_eq!(*acc.identity().state(), AccountState::PENDING);
            assert_eq!(acc.version(), version_snapshot);
        })
        .await?;

        f.assert_outbox(0, None);

        Ok(())
    }
}
