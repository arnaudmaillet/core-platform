#[cfg(test)]
mod tests {
    use crate::application::use_cases::moderation::decrease_trust_score::{
        DecreaseTrustScoreCommand, DecreaseTrustScoreHandler,
    };
    use crate::application::utils::TestFixture;
    use crate::domain::events::AccountEvent;
    use crate::domain::value_objects::{AccountState, TrustDelta, TrustScore};
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::value_objects::{AuditReason, RegionCode};
    use shared_kernel::errors::{DomainError, Result};
    use uuid::Uuid;

    #[tokio::test]
    async fn test_decrease_trust_score_success() -> Result<()> {
        let f = TestFixture::new();

        // 1. Arrange : Compte actif (score 100 par défaut via builder)
        let account = f
            .account_builder()?
            .with_state(AccountState::Active)
            .build()?;

        let version_snapshot = account.version();
        f.account_repo().insert(account);

        let cmd = DecreaseTrustScoreCommand {
            command_id: Uuid::new_v4(),
            account_id: f.account_id(),
            amount: TrustDelta::from_raw(30),
            reason: AuditReason::try_new("Suspicious activity")?,
        };

        // 2. Act
        f.bus()
            .execute(f.account_ctx(), cmd, DecreaseTrustScoreHandler)
            .await?;

        // 3. Assert
        f.assert_account(|acc| {
            assert_eq!(acc.governance().trust_score().value(), 70);
            assert_eq!(acc.version(), version_snapshot + 1);
        })
        .await?;

        // Vérification de l'événement exact produit par penalize_trust
        f.assert_outbox(1, Some(AccountEvent::TRUST_SCORE_ADJUSTED));

        Ok(())
    }

    #[tokio::test]
    async fn test_decrease_trust_score_clamping_and_shadowban() -> Result<()> {
        let f = TestFixture::new();

        // 1. Arrange : Utilisation du builder avec un score précis
        let account = f
            .account_builder()?
            .with_state(AccountState::Active)
            .with_trust_score(TrustScore::from_raw(TrustScore::CRITICAL_THRESHOLD))
            .build()?;

        let version_snapshot = account.version();
        f.account_repo().insert(account);

        let cmd = DecreaseTrustScoreCommand {
            command_id: Uuid::new_v4(),
            account_id: f.account_id(),
            amount: TrustDelta::from_raw(50), // 20 - 50 -> Clamping à 0
            reason: AuditReason::try_new("Heavy violation")?,
        };

        // 2. Act
        f.bus()
            .execute(f.account_ctx(), cmd, DecreaseTrustScoreHandler)
            .await?;

        // 3. Assert
        f.assert_account(|acc| {
            assert_eq!(acc.governance().trust_score().value(), 0);
            assert!(acc.governance().is_shadowbanned());
            // v1 + 1 (score) + 1 (shadowban) = v3
            assert_eq!(acc.version(), version_snapshot + 1);
        })
        .await?;

        f.assert_outbox(2, None); // ScoreAdjusted + ShadowbanUpdated

        Ok(())
    }

    #[tokio::test]
    async fn test_decrease_trust_score_technical_idempotency() -> Result<()> {
        let f = TestFixture::new();
        let cmd_id = Uuid::new_v4();

        f.idempotency_repo().seed(cmd_id);

        let account = f
            .account_builder()?
            .with_state(AccountState::Active)
            .build()?;
        let version_snapshot = account.version();
        f.account_repo().insert(account);

        let cmd = DecreaseTrustScoreCommand {
            command_id: cmd_id,
            account_id: f.account_id(),
            amount: TrustDelta::from_raw(10),
            reason: AuditReason::try_new("Duplicate")?,
        };

        let result: std::result::Result<(), DomainError> = f
            .bus()
            .execute(f.account_ctx(), cmd, DecreaseTrustScoreHandler)
            .await;

        assert!(matches!(result, Err(DomainError::AlreadyExists { .. })));

        f.assert_account(|acc| {
            assert_eq!(acc.version(), version_snapshot);
        })
        .await?;

        Ok(())
    }

    #[tokio::test]
    async fn test_decrease_trust_score_business_idempotency_at_floor() -> Result<()> {
        let f = TestFixture::new();

        // Arrange : Utilisation du with_state(Banned) qui met auto le score à 0 et shadowban
        let account = f
            .account_builder()?
            .with_state(AccountState::Banned)
            .build()?;

        let version_snapshot = account.version();
        f.account_repo().insert(account);

        let cmd = DecreaseTrustScoreCommand {
            command_id: Uuid::new_v4(),
            account_id: f.account_id(),
            amount: TrustDelta::from_raw(10),
            reason: AuditReason::try_new("Already at zero")?,
        };

        // 2. Act
        f.bus()
            .execute(f.account_ctx(), cmd, DecreaseTrustScoreHandler)
            .await?;

        // 3. Assert
        f.assert_account(|acc| {
            assert_eq!(acc.governance().trust_score().value(), 0);
            assert_eq!(
                acc.version(),
                version_snapshot,
                "Pas de mutation si déjà au plancher"
            );
        })
        .await?;

        f.assert_outbox(0, None);

        Ok(())
    }

    #[tokio::test]
    async fn test_trust_decrease_succeeds_after_retry() -> Result<()> {
        let f = TestFixture::new();
        let account = f
            .account_builder()?
            .with_state(AccountState::Active)
            .build()?;
        f.account_repo().insert(account);

        // On simule UNE erreur (le stub la supprimera après le premier essai)
        f.account_repo()
            .set_error_once(DomainError::ConcurrencyConflict {
                reason: "DB Busy".into(),
            });

        let cmd = DecreaseTrustScoreCommand {
            command_id: Uuid::new_v4(),
            account_id: f.account_id(),
            amount: TrustDelta::from_raw(1),
            reason: AuditReason::try_new("Test")?,
        };

        // ACT : Le bus doit absorber l'erreur et réussir au second essai
        let result = f
            .bus()
            .execute(f.account_ctx(), cmd, DecreaseTrustScoreHandler)
            .await;

        // ASSERT
        assert!(result.is_ok(), "Le retry aurait dû sauver l'opération");
        f.assert_outbox(1, Some(AccountEvent::TRUST_SCORE_ADJUSTED));
        Ok(())
    }

    #[tokio::test]
    async fn test_region_mismatch_returns_not_found() -> Result<()> {
        let f = TestFixture::new();
        let wrong_region = RegionCode::from_raw("us");

        // Compte aux US, Contexte en EU
        let account = f
            .account_builder_for(wrong_region)?
            .with_state(AccountState::Active)
            .build()?;
        let version_snapshot = account.version();
        f.account_repo().insert(account);

        let cmd = DecreaseTrustScoreCommand {
            command_id: Uuid::new_v4(),
            account_id: f.account_id(),
            amount: TrustDelta::from_raw(1),
            reason: AuditReason::try_new("Test")?,
        };

        let result = f
            .bus()
            .execute(f.account_ctx(), cmd, DecreaseTrustScoreHandler)
            .await;
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
