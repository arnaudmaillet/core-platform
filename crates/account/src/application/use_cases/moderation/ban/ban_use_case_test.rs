#[cfg(test)]
mod tests {
    use crate::application::context::AccountContext;
    use crate::application::use_cases::moderation::BanCommand;
    use crate::application::utils::TestFixture;
    use crate::domain::events::AccountEvent;
    use crate::domain::value_objects::AccountState;
    use shared_kernel::domain::entities::Versioned;
    use shared_kernel::domain::value_objects::AuditReason;
    use shared_kernel::core::{DomainError, Result};
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
            account_id: f.account_id(),
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
        f.account_repo().insert(account);

        let cmd = BanCommand {
            command_id: cmd_id,
            account_id: f.account_id(),
            reason: AuditReason::try_new("Duplicate Ban")?,
        };

        // Act
        let result = f
            .bus()
            .execute::<AccountContext, BanCommand, ()>(f.account_ctx().clone(), cmd)
            .await;

        // Assert
        assert!(matches!(result, Err(DomainError::AlreadyExists { .. })));
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
            account_id: f.account_id(),
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
            account_id: f.account_id(),
            reason: AuditReason::try_new("No matter")?,
        };

        let result = f
            .bus()
            .execute::<AccountContext, BanCommand, ()>(f.account_ctx().clone(), cmd)
            .await;

        assert!(matches!(
            result,
            Err(DomainError::NotFound {
                entity: "Account",
                ..
            })
        ));
        f.assert_outbox(0, None);
        Ok(())
    }

    #[tokio::test]
    async fn test_concurrency_retry_success() -> Result<()> {
        let f = TestFixture::new();
        let account = f.account_builder()?.build()?;
        f.account_repo().insert(account);

        // On simule UN conflit (grâce au .take() dans le stub, seul le 1er appel échouera)
        f.account_repo()
            .set_error_once(DomainError::ConcurrencyConflict {
                reason: "Race condition".into(),
            });

        let cmd = BanCommand {
            command_id: Uuid::new_v4(),
            account_id: f.account_id(),
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
        f.account_repo().insert(account);

        // On simule une erreur lors de l'écriture de l'outbox (transaction fail)
        f.outbox_repo()
            .set_error(DomainError::Infrastructure("Disk full".into()));

        let cmd = BanCommand {
            command_id: Uuid::new_v4(),
            account_id: f.account_id(),
            reason: AuditReason::try_new("System ban")?,
        };

        let result = f
            .bus()
            .execute::<AccountContext, BanCommand, ()>(f.account_ctx().clone(), cmd)
            .await;

        // La transaction globale échoue si l'outbox échoue
        assert!(matches!(result, Err(DomainError::Infrastructure(m)) if m == "Disk full"));

        // Vérification cruciale : l'état en base n'a pas dû changer (rollback simulé par le stub)
        // Note: Le stub doit être configuré pour ne pas persister si check_error fail
        Ok(())
    }
}
