#[cfg(test)]
mod tests {
    use crate::application::commands::lifecycle::ChangeBetaTierCommand;
    use crate::application::context::AccountContext;
    use crate::application::utils::TestFixture;
    use crate::domain::events::AccountEvent;
    use crate::domain::types::BetaTier;
    use shared_kernel::{
        command::CommandTarget,
        core::{Result, Versioned},
        messaging::EventEmitter,
    };
    use uuid::Uuid;

    #[tokio::test]
    async fn test_change_beta_tier_success() -> Result<()> {
        let f = TestFixture::new();
        let account = f.account_builder()?.build()?;
        let version_snapshot = account.version();

        f.account_repo().insert(account);

        let cmd = ChangeBetaTierCommand {
            command_id: Uuid::new_v4(),
            target: CommandTarget::new(f.account_id(), f.region(), version_snapshot),
            new_tier: BetaTier::BETA,
        };

        f.bus()
            .execute::<AccountContext, ChangeBetaTierCommand, ()>(f.account_ctx().clone(), cmd)
            .await?;

        f.assert_account(|acc| {
            assert_eq!(acc.governance().beta_tier(), BetaTier::BETA);
            assert_eq!(acc.version(), version_snapshot + 1);
        })
        .await?;

        f.assert_outbox(1, Some(AccountEvent::BETA_TIER_CHANGED));

        Ok(())
    }

    #[tokio::test]
    async fn test_change_beta_tier_technical_idempotency() -> Result<()> {
        let f = TestFixture::new();
        let cmd_id = Uuid::new_v4();

        // On simule une commande déjà traitée techniquement
        f.idempotency_repo().seed(cmd_id);

        let mut account = f.account_builder()?.build()?;
        // On initialise l'état pour le test
        let _ = account.change_beta_tier(BetaTier::ALPHA);
        account.pull_events();

        let version_snapshot = account.version();
        f.account_repo().insert(account);

        let cmd = ChangeBetaTierCommand {
            command_id: cmd_id,
            target: CommandTarget::new(f.account_id(), f.region(), version_snapshot),
            new_tier: BetaTier::ALPHA,
        };

        let result = f
            .bus()
            .execute::<AccountContext, ChangeBetaTierCommand, ()>(f.account_ctx().clone(), cmd)
            .await;

        // Doit retourner une erreur d'idempotence technique
        assert!(
            result.is_ok(),
            "L'idempotence technique doit être transparente (Ok)"
        );
        f.assert_account(|acc| {
            assert_eq!(acc.governance().beta_tier(), BetaTier::ALPHA);
            assert_eq!(acc.version(), version_snapshot);
        })
        .await?;

        f.assert_outbox(0, None);

        Ok(())
    }

    #[tokio::test]
    async fn test_change_beta_tier_business_idempotency() -> Result<()> {
        let f = TestFixture::new();
        let mut account = f.account_builder()?.build()?;

        // On le passe déjà en ALPHA
        let _ = account.change_beta_tier(BetaTier::ALPHA);
        account.pull_events();

        let version_snapshot = account.version();
        f.account_repo().insert(account);

        let cmd = ChangeBetaTierCommand {
            command_id: Uuid::new_v4(),
            target: CommandTarget::new(f.account_id(), f.region(), version_snapshot),
            new_tier: BetaTier::ALPHA, // On redemande ALPHA
        };

        // L'exécution doit réussir mais ne rien changer
        f.bus()
            .execute::<AccountContext, ChangeBetaTierCommand, ()>(f.account_ctx().clone(), cmd)
            .await?;

        f.assert_account(|acc| {
            assert_eq!(acc.governance().beta_tier(), BetaTier::ALPHA);
            assert_eq!(acc.version(), version_snapshot); // Pas d'incrément
        })
        .await?;

        f.assert_outbox(0, None); // Pas d'événement produit
        Ok(())
    }
}
