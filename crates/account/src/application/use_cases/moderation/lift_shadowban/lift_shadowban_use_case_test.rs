#[cfg(test)]
mod tests {
    use crate::application::context::AccountContext;
    use crate::application::use_cases::moderation::{
        LiftShadowbanCommand, LiftShadowbanHandler,
    };
    use crate::application::utils::TestFixture;
    use crate::domain::events::AccountEvent;
    use crate::domain::value_objects::AccountState;
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::value_objects::{AuditReason, RegionCode};
    use shared_kernel::errors::{DomainError, Result};
    use uuid::Uuid;

    #[tokio::test]
    async fn test_lift_shadowban_success() -> Result<()> {
        let f = TestFixture::new();

        // 1. Arrange : Un compte banni est automatiquement shadowbanné par notre builder
        let account = f
            .account_builder()?
            .with_state(AccountState::Banned)
            .build()?;

        let version_snapshot = account.version();
        f.account_repo().insert(account);

        let cmd = LiftShadowbanCommand {
            command_id: Uuid::new_v4(),
            account_id: f.account_id(),
            reason: AuditReason::try_new("Appeal accepted")?,
        };

        // 2. Act
        f.bus()
            .execute::<AccountContext, LiftShadowbanCommand, ()>(f.account_ctx().clone(), cmd)
            .await?;

        // 3. Assert
        f.assert_account(|acc| {
            assert!(
                !acc.governance().is_shadowbanned(),
                "Le shadowban doit être levé"
            );
            assert_eq!(acc.version(), version_snapshot + 1);
        })
        .await?;

        f.assert_outbox(1, Some(AccountEvent::SHADOWBAN_UPDATED));

        Ok(())
    }

    #[tokio::test]
    async fn test_lift_shadowban_technical_idempotency() -> Result<()> {
        let f = TestFixture::new();
        let cmd_id = Uuid::new_v4();

        // Arrange
        f.idempotency_repo().seed(cmd_id);

        let account = f
            .account_builder()?
            .with_state(AccountState::Banned)
            .build()?;
        f.account_repo().insert(account);

        let cmd = LiftShadowbanCommand {
            command_id: cmd_id,
            account_id: f.account_id(),
            reason: AuditReason::try_new("Duplicate call")?,
        };

        // Act
        let result = f
            .bus()
            .execute::<AccountContext, LiftShadowbanCommand, ()>(f.account_ctx().clone(), cmd)
            .await;

        // Assert
        assert!(matches!(result, Err(DomainError::AlreadyExists { .. })));
        f.assert_outbox(0, None);

        Ok(())
    }

    #[tokio::test]
    async fn test_lift_shadowban_business_idempotency() -> Result<()> {
        let f = TestFixture::new();

        // 1. Arrange : Compte déjà sain (Shadowban = false par défaut)
        let account = f
            .account_builder()?
            .with_state(AccountState::Active)
            .build()?;
        let version_snapshot = account.version();
        f.account_repo().insert(account);

        let cmd = LiftShadowbanCommand {
            command_id: Uuid::new_v4(),
            account_id: f.account_id(),
            reason: AuditReason::try_new("Accidental click")?,
        };

        // 2. Act
        f.bus()
            .execute::<AccountContext, LiftShadowbanCommand, ()>(f.account_ctx().clone(), cmd)
            .await?;

        // 3. Assert
        f.assert_account(|acc| {
            assert!(!acc.governance().is_shadowbanned());
            assert_eq!(
                acc.version(),
                version_snapshot,
                "La version ne doit pas augmenter si aucun changement"
            );
        })
        .await?;

        f.assert_outbox(0, None);

        Ok(())
    }

    #[tokio::test]
    async fn test_region_mismatch_returns_not_found() -> Result<()> {
        let f = TestFixture::new();
        let wrong_region = RegionCode::from_raw("us");

        // Arrange
        let account = f
            .account_builder_for(wrong_region)?
            .with_state(AccountState::Banned)
            .build()?;

        let version_snapshot = account.version();
        f.account_repo().insert(account);

        let cmd = LiftShadowbanCommand {
            command_id: Uuid::new_v4(),
            account_id: f.account_id(),
            reason: AuditReason::try_new("Accidental click")?,
        };

        // Act
        let result = f
            .bus()
            .execute::<AccountContext, LiftShadowbanCommand, ()>(f.account_ctx().clone(), cmd)
            .await;

        // Assert
        let saved = f.account_repo().find_direct(&f.account_id()).unwrap();
        assert!(matches!(result, Err(DomainError::NotFound { .. })));

        assert_eq!(
            saved.version(),
            version_snapshot,
            "La version ne doit pas avoir bougé"
        );

        f.assert_outbox(0, None);
        Ok(())
    }
}
