// crates/account/src/application/use_cases/lifecycle/change_beta_tier/change_beta_tier_use_case_test.rs

use account_old::commands::lifecycle::ChangeBetaTierCommand;
use account_old::context::AccountCommandCtx;
use account_old::events::AccountEvent;
use account_old::types::BetaTier;
use account_test_utils::asserts::AccountRepositoryAsserts;

use account_test_utils::AccountTestFixture;
use shared_kernel::{
    command::CommandTarget,
    core::{Result, Versioned},
    idempotency::IdempotencyRepository,
    messaging::EventEmitter,
};
use uuid::Uuid;

#[tokio::test]
async fn test_change_beta_tier_success() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();
    let account = f.builder()?.build()?;
    let version_snapshot = account.version();
    f.account_repo().insert(account);

    let cmd = ChangeBetaTierCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.account_id(), version_snapshot),
        region: f.server_region(),
        new_tier: BetaTier::BETA,
    };

    // Act
    f.bus()
        .execute::<AccountCommandCtx, ChangeBetaTierCommand, ()>(f.command_ctx().clone(), cmd)
        .await?;

    // Assert
    f.account_assertions()
        .assert_account_state(f.account_id(), |acc| {
            assert_eq!(acc.governance().beta_tier(), BetaTier::BETA);
            assert_eq!(acc.version(), version_snapshot + 1);
        })
        .await;

    f.assert_outbox(1, Some(AccountEvent::BETA_TIER_CHANGED));

    Ok(())
}

#[tokio::test]
async fn test_change_beta_tier_technical_idempotency() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();
    let cmd_id = Uuid::new_v4();

    // On simule une commande déjà interceptée par le premier rideau d'idempotence
    f.idempotency_repo().save(None, &cmd_id).await?;

    let mut account = f.builder()?.build()?;
    let _ = account.change_beta_tier(BetaTier::ALPHA);
    account.pull_events(); // On vide l'outbox locale du setup

    let version_snapshot = account.version();
    f.account_repo().insert(account);

    let cmd = ChangeBetaTierCommand {
        command_id: cmd_id,
        target: CommandTarget::versioned(f.account_id(), version_snapshot),
        region: f.server_region(),
        new_tier: BetaTier::ALPHA,
    };

    // Act
    let result = f
        .bus()
        .execute::<AccountCommandCtx, ChangeBetaTierCommand, ()>(f.command_ctx().clone(), cmd)
        .await;

    // Assert
    assert!(
        result.is_ok(),
        "L'idempotence technique doit court-circuiter de façon transparente (Ok)"
    );

    // L'état en base n'a pas bougé
    f.account_assertions()
        .assert_account_state(f.account_id(), |acc| {
            assert_eq!(acc.governance().beta_tier(), BetaTier::ALPHA);
            assert_eq!(acc.version(), version_snapshot);
        })
        .await;

    // Aucun événement dupliqué n'est poussé dans le journal Outbox
    f.account_assertions()
        .assert_no_events_for(f.account_id())
        .await;

    Ok(())
}

#[tokio::test]
async fn test_change_beta_tier_business_idempotency() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();
    let mut account = f.builder()?.build()?;

    // Idempotence métier : Le compte est déjà configuré sur le tier ALPHA
    let _ = account.change_beta_tier(BetaTier::ALPHA);
    account.pull_events();

    let version_snapshot = account.version();
    f.account_repo().insert(account);

    let cmd = ChangeBetaTierCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.account_id(), version_snapshot),
        region: f.server_region(),
        new_tier: BetaTier::ALPHA, // On demande le même état
    };

    // Act
    f.bus()
        .execute::<AccountCommandCtx, ChangeBetaTierCommand, ()>(f.command_ctx().clone(), cmd)
        .await?;

    // Assert
    // L'opération s'est exécutée avec succès mais n'a appliqué aucune mutation (No-Op transactionnel)
    f.account_assertions()
        .assert_account_state(f.account_id(), |acc| {
            assert_eq!(acc.governance().beta_tier(), BetaTier::ALPHA);
            assert_eq!(acc.version(), version_snapshot);
        })
        .await;

    // Aucun événement métier n'a été produit puisque l'état est inchangé
    f.account_assertions()
        .assert_no_events_for(f.account_id())
        .await;

    Ok(())
}
