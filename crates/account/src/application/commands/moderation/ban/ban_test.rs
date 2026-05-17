#[cfg(test)]
mod tests {
    use crate::application::commands::moderation::BanCommand;
    use crate::application::context::AccountContext;
    use crate::application::utils::TestFixture;
    use crate::domain::events::AccountEvent;
    use crate::domain::types::AccountState;
    use shared_kernel::command::CommandTarget;
    use shared_kernel::core::{Error, ErrorCode, Result, Versioned};
    use shared_kernel::types::AuditReason;
    use uuid::Uuid;

    #[tokio::test]
    async fn test_ban_account_success() -> Result<()> {
        let f = TestFixture::new();

        // 1. Arrange : Compte actif
        let account = f.account_builder()?.build()?;
        let version_snapshot = account.version();
        f.account_repo().insert(account);

        let cmd = BanCommand {
            command_id: Uuid::new_v4(),
            target: CommandTarget::new(f.account_id(), f.region(), version_snapshot),
            reason: AuditReason::try_new("TOS Violation")?,
        };

        // 2. Act
        f.bus()
            .execute::<AccountContext, BanCommand, ()>(f.account_ctx().clone(), cmd)
            .await?;

        // 3. Assert
        f.assert_account(|acc| {
            assert_eq!(*acc.identity().state(), AccountState::BANNED);
            assert_eq!(acc.version(), version_snapshot + 1);
        })
        .await?;

        f.assert_outbox(1, Some(AccountEvent::BANNED));

        Ok(())
    }

    #[tokio::test]
    async fn test_ban_technical_idempotency() -> Result<()> {
        let f = TestFixture::new();
        let cmd_id = Uuid::new_v4();

        f.idempotency_repo().seed(cmd_id);

        let account = f.account_builder()?.build()?;
        let version_snapshot = account.version();
        f.account_repo().insert(account);

        let cmd = BanCommand {
            command_id: cmd_id,
            target: CommandTarget::new(f.account_id(), f.region(), version_snapshot),
            reason: AuditReason::try_new("Duplicate Ban")?,
        };

        // Act
        let result = f
            .bus()
            .execute::<AccountContext, BanCommand, ()>(f.account_ctx().clone(), cmd)
            .await;

        // Assert
        assert!(
            result.is_ok(),
            "L'idempotence technique doit être transparente (Ok)"
        );
        f.assert_outbox(0, None);

        Ok(())
    }

    #[tokio::test]
    async fn test_ban_business_idempotency() -> Result<()> {
        let f = TestFixture::new();

        // Arrange : Compte déjà Banni
        let mut account = f.account_builder()?.build()?;
        account.ban(AuditReason::try_new("First reason")?)?;

        let version_snapshot = account.version();
        f.account_repo().insert(account);

        let cmd = BanCommand {
            command_id: Uuid::new_v4(),
            target: CommandTarget::new(f.account_id(), f.region(), version_snapshot),
            reason: AuditReason::try_new("Second attempt")?,
        };

        // Act
        f.bus()
            .execute::<AccountContext, BanCommand, ()>(f.account_ctx().clone(), cmd)
            .await?;

        // Assert
        f.assert_account(|acc| {
            assert_eq!(*acc.identity().state(), AccountState::BANNED);
            assert_eq!(acc.version(), version_snapshot);
        })
        .await?;

        f.assert_outbox(0, None);

        Ok(())
    }

    #[tokio::test]
    async fn test_worst_case_account_not_found() -> Result<()> {
        let f = TestFixture::new();

        let cmd = BanCommand {
            command_id: Uuid::new_v4(),
            target: CommandTarget::new(f.account_id(), f.region(), 0),
            reason: AuditReason::try_new("Violating TOS")?,
        };

        // 2. Act
        let result = f
            .bus()
            .execute::<AccountContext, BanCommand, ()>(f.account_ctx().clone(), cmd)
            .await;

        // 3. Assert
        match result {
            Err(e) => {
                assert_eq!(e.code, ErrorCode::NotFound);
                assert!(e.message.contains("Account"));
            }
            Ok(_) => panic!("Should have failed: Account does not exist"),
        }

        f.assert_outbox(0, None);
        Ok(())
    }

    #[tokio::test]
    async fn test_concurrency_retry_success() -> Result<()> {
        let f = TestFixture::new();
        let account = f.account_builder()?.build()?;
        let version_snapshot = account.version();
        f.account_repo().insert(account);

        // On simule UN conflit (grâce au .take() dans le stub, seul le 1er appel échouera)
        f.account_repo()
            .set_error_once(Error::concurrency_conflict("Race condition"));

        let cmd = BanCommand {
            command_id: Uuid::new_v4(),
            target: CommandTarget::new(f.account_id(), f.region(), version_snapshot),
            reason: AuditReason::try_new("System ban")?,
        };

        // ACT : Le bus doit absorber l'erreur et réussir au 2ème essai
        let result = f
            .bus()
            .execute::<AccountContext, BanCommand, ()>(f.account_ctx().clone(), cmd)
            .await;

        // ASSERT
        assert!(result.is_ok(), "Le bus aurait dû réussir après un retry");
        f.assert_account(|acc| {
            assert!(acc.identity().is_banned());
        })
        .await?;

        Ok(())
    }

    #[tokio::test]
    async fn test_worst_case_atomic_outbox_failure() -> Result<()> {
        let f = TestFixture::new();

        let account = f.account_builder()?.build()?;
        let version_snapshot = account.version();
        f.account_repo().insert(account);

        // On simule une erreur lors de l'écriture de l'outbox (transaction fail)
        f.outbox_repo().set_error(Error::internal("Disk full"));

        let cmd = BanCommand {
            command_id: Uuid::new_v4(),
            target: CommandTarget::new(f.account_id(), f.region(), version_snapshot),
            reason: AuditReason::try_new("System ban")?,
        };

        let result = f
            .bus()
            .execute::<AccountContext, BanCommand, ()>(f.account_ctx().clone(), cmd)
            .await;

        // La transaction globale échoue si l'outbox échoue
        match result {
            Err(e) => {
                assert_eq!(e.code, ErrorCode::InternalError);
            }
            Ok(_) => panic!("Should have failed with internal error"),
        }

        // Vérification cruciale : l'état en base n'a pas dû changer (rollback simulé par le stub)
        // Note: Le stub doit être configuré pour ne pas persister si check_error fail
        Ok(())
    }
}
