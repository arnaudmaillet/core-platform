#[cfg(test)]
mod tests {
    use crate::application::context::AccountContext;
    use crate::application::use_cases::moderation::{
        IncreaseTrustScoreCommand, IncreaseTrustScoreHandler,
    };
    use crate::application::utils::TestFixture;
    use crate::domain::events::AccountEvent;
    use crate::domain::value_objects::{AccountState, TrustDelta, TrustScore};
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::value_objects::{AuditReason, RegionCode};
    use shared_kernel::errors::{DomainError, Result};
    use uuid::Uuid;

    #[tokio::test]
    async fn test_increase_trust_score_success() -> Result<()> {
        let f = TestFixture::new();

        // 1. Arrange : Score initial à 50
        let account = f
            .account_builder()?
            .with_state(AccountState::ACTIVE)
            .with_trust_score(TrustScore::from_raw(50))
            .build()?;

        let version_snapshot = account.version();
        f.account_repo().insert(account);

        let cmd = IncreaseTrustScoreCommand {
            command_id: Uuid::new_v4(),
            account_id: f.account_id(),
            amount: TrustDelta::from_raw(20), // 50 + 20 = 70
            reason: AuditReason::try_new("Good behavior")?,
        };

        // 2. Act
        f.bus()
            .execute::<AccountContext, IncreaseTrustScoreCommand, ()>(f.account_ctx().clone(), cmd)
            .await?;

        // 3. Assert
        f.assert_account(|acc| {
            assert_eq!(acc.governance().trust_score().value(), 70);
            assert_eq!(acc.version(), version_snapshot + 1);
        })
        .await?;

        f.assert_outbox(1, Some(AccountEvent::TRUST_SCORE_ADJUSTED));

        Ok(())
    }

    #[tokio::test]
    async fn test_increase_trust_score_cap_at_one_hundred() -> Result<()> {
        let f = TestFixture::new();

        // 1. Arrange : Score à 90
        let account = f
            .account_builder()?
            .with_state(AccountState::ACTIVE)
            .with_trust_score(TrustScore::from_raw(50))
            .build()?;

        let version_snapshot = account.version();
        f.account_repo().insert(account);

        let cmd = IncreaseTrustScoreCommand {
            command_id: Uuid::new_v4(),
            account_id: f.account_id(),
            amount: TrustDelta::from_raw(50), // 90 + 50 -> Saturé à 100
            reason: AuditReason::try_new("High activity")?,
        };

        // 2. Act
        f.bus()
            .execute::<AccountContext, IncreaseTrustScoreCommand, ()>(f.account_ctx().clone(), cmd)
            .await?;

        // 3. Assert
        f.assert_account(|acc| {
            assert_eq!(acc.governance().trust_score().value(), 100);
            assert_eq!(acc.version(), version_snapshot + 1);
        })
        .await?;

        Ok(())
    }

    #[tokio::test]
    async fn test_increase_trust_score_technical_idempotency() -> Result<()> {
        let f = TestFixture::new();
        let cmd_id = Uuid::new_v4();

        // Arrange : Infrastructure connaît déjà la commande
        f.idempotency_repo().seed(cmd_id);

        let account = f
            .account_builder()?
            .with_state(AccountState::ACTIVE)
            .build()?;
        let version_snapshot = account.version();
        f.account_repo().insert(account);

        let cmd = IncreaseTrustScoreCommand {
            command_id: cmd_id,
            account_id: f.account_id(),
            amount: TrustDelta::from_raw(10),
            reason: AuditReason::try_new("Duplicate")?,
        };

        // 2. Act
        let result = f
            .bus()
            .execute::<AccountContext, IncreaseTrustScoreCommand, ()>(f.account_ctx().clone(), cmd)
            .await;

        f.assert_account(|acc| {
            assert_eq!(acc.governance().trust_score().value(), 100);
            assert_eq!(acc.version(), version_snapshot);
        })
        .await?;

        // 3. Assert
        assert!(matches!(result, Err(DomainError::AlreadyExists { .. })));
        f.assert_outbox(0, None);

        Ok(())
    }

    #[tokio::test]
    async fn test_increase_trust_score_business_idempotency_at_max() -> Result<()> {
        let f = TestFixture::new();

        // Arrange : Déjà au maximum (100)
        let account = f
            .account_builder()?
            .with_state(AccountState::ACTIVE)
            .with_trust_score(TrustScore::from_raw(TrustScore::MAX))
            .build()?;

        let version_snapshot = account.version();
        f.account_repo().insert(account);

        let cmd = IncreaseTrustScoreCommand {
            command_id: Uuid::new_v4(),
            account_id: f.account_id(),
            amount: TrustDelta::from_raw(10),
            reason: AuditReason::try_new("Should do nothing")?,
        };

        // 2. Act
        f.bus()
            .execute::<AccountContext, IncreaseTrustScoreCommand, ()>(f.account_ctx().clone(), cmd)
            .await?;

        // 3. Assert
        f.assert_account(|acc| {
            assert_eq!(acc.governance().trust_score().value(), 100);
            assert_eq!(
                acc.version(),
                version_snapshot,
                "La version ne doit pas bouger"
            );
        })
        .await?;

        f.assert_outbox(0, None);

        Ok(())
    }

    #[tokio::test]
    async fn test_region_mismatch_returns_not_found() -> Result<()> {
        let f = TestFixture::new();
        let wrong_region = RegionCode::from_raw("US");

        // Arrange : Compte US vs Contexte EU
        let account = f
            .account_builder_for(wrong_region)?
            .with_state(AccountState::ACTIVE)
            .build()?;
        let version_snapshot = account.version();

        f.account_repo().insert(account);

        let cmd = IncreaseTrustScoreCommand {
            command_id: Uuid::new_v4(),
            account_id: f.account_id(),
            amount: TrustDelta::from_raw(10),
            reason: AuditReason::try_new("No matter")?,
        };

        // Act
        let result = f
            .bus()
            .execute::<AccountContext, IncreaseTrustScoreCommand, ()>(f.account_ctx().clone(), cmd)
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
