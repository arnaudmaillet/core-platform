#[cfg(test)]
mod tests {
    use crate::application::commands::moderation::DecreaseTrustScoreCommand;
    use crate::application::context::AccountContext;
    use crate::application::utils::TestFixture;
    use crate::domain::events::AccountEvent;
    use crate::domain::types::{AccountState, TrustAmount, TrustScore};
    use shared_kernel::command::CommandTarget;
    use shared_kernel::core::{Error, Result, Versioned};
    use shared_kernel::types::AuditReason;
    use uuid::Uuid;

    #[tokio::test]
    async fn test_decrease_trust_score_success() -> Result<()> {
        let f = TestFixture::new();

        // 1. Arrange : Compte actif (score 100 par défaut via builder)
        let account = f
            .account_builder()?
            .with_state(AccountState::ACTIVE)
            .build()?;

        let version_snapshot = account.version();
        f.account_repo().insert(account);

        let cmd = DecreaseTrustScoreCommand {
            command_id: Uuid::new_v4(),
            target: CommandTarget::new(f.account_id(), f.region(), version_snapshot),
            amount: TrustAmount::try_from(30)?,
            reason: AuditReason::try_new("Suspicious activity")?,
        };

        // 2. Act
        f.bus()
            .execute::<AccountContext, DecreaseTrustScoreCommand, ()>(f.account_ctx().clone(), cmd)
            .await?;

        // 3. Assert
        f.assert_account(|acc| {
            assert_eq!(acc.governance().trust_score().value(), 70);
            assert_eq!(acc.version(), version_snapshot + 1);
        })
        .await?;

        // Vérification de l'événement exact produit par penalize_trust
        f.assert_outbox(1, Some(AccountEvent::TRUST_SCORE_PENALIZED));

        Ok(())
    }

    #[tokio::test]
    async fn test_decrease_trust_score_clamping_and_shadowban() -> Result<()> {
        let f = TestFixture::new();

        // 1. Arrange : Utilisation du builder avec un score précis
        let account = f
            .account_builder()?
            .with_state(AccountState::ACTIVE)
            .with_trust_score(TrustScore::from_raw(TrustScore::CRITICAL_THRESHOLD))
            .build()?;

        let version_snapshot = account.version();
        f.account_repo().insert(account);

        let cmd = DecreaseTrustScoreCommand {
            command_id: Uuid::new_v4(),
            target: CommandTarget::new(f.account_id(), f.region(), version_snapshot),
            amount: TrustAmount::try_from(50)?, // 20 - 50 -> Clamping à 0
            reason: AuditReason::try_new("Heavy violation")?,
        };

        // 2. Act
        f.bus()
            .execute::<AccountContext, DecreaseTrustScoreCommand, ()>(f.account_ctx().clone(), cmd)
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
            .with_state(AccountState::ACTIVE)
            .build()?;
        let version_snapshot = account.version();
        f.account_repo().insert(account);

        let cmd = DecreaseTrustScoreCommand {
            command_id: cmd_id,
            target: CommandTarget::new(f.account_id(), f.region(), version_snapshot),
            amount: TrustAmount::try_from(10)?,
            reason: AuditReason::try_new("Duplicate")?,
        };

        let result: std::result::Result<(), Error> = f
            .bus()
            .execute::<AccountContext, DecreaseTrustScoreCommand, ()>(f.account_ctx().clone(), cmd)
            .await;

        assert!(
            result.is_ok(),
            "L'idempotence technique doit être transparente (Ok)"
        );

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
            .with_state(AccountState::BANNED)
            .build()?;

        let version_snapshot = account.version();
        f.account_repo().insert(account);

        let cmd = DecreaseTrustScoreCommand {
            command_id: Uuid::new_v4(),
            target: CommandTarget::new(f.account_id(), f.region(), version_snapshot),
            amount: TrustAmount::try_from(10)?,
            reason: AuditReason::try_new("Already at zero")?,
        };

        // 2. Act
        f.bus()
            .execute::<AccountContext, DecreaseTrustScoreCommand, ()>(f.account_ctx().clone(), cmd)
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
            .with_state(AccountState::ACTIVE)
            .build()?;
        let version_snapshot = account.version();
        f.account_repo().insert(account);

        // On simule UNE erreur (le stub la supprimera après le premier essai)
        f.account_repo()
            .set_error_once(Error::concurrency_conflict("DB Busy"));

        let cmd = DecreaseTrustScoreCommand {
            command_id: Uuid::new_v4(),
            target: CommandTarget::new(f.account_id(), f.region(), version_snapshot),
            amount: TrustAmount::try_from(1)?,
            reason: AuditReason::try_new("Test")?,
        };

        // ACT : Le bus doit absorber l'erreur et réussir au second essai
        let result = f
            .bus()
            .execute::<AccountContext, DecreaseTrustScoreCommand, ()>(f.account_ctx().clone(), cmd)
            .await;

        // ASSERT
        assert!(result.is_ok(), "Le retry aurait dû sauver l'opération");
        f.assert_outbox(1, Some(AccountEvent::TRUST_SCORE_PENALIZED));
        Ok(())
    }
}
