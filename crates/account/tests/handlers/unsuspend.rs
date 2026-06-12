// crates/account/src/application/use_cases/lifecycle/unsuspend/unsuspend_use_case_test.rs

use account::commands::lifecycle::UnsuspendCommand;
use account::context::AccountCommandCtx;
use account::events::AccountEvent;
use account::types::AccountState;
use account_test_utils::asserts::AccountRepositoryAsserts;

use account_test_utils::AccountTestFixture;
use shared_kernel::command::CommandTarget;
use shared_kernel::core::{Result, Versioned};
use shared_kernel::idempotency::IdempotencyRepository;
use shared_kernel::messaging::EventEmitter;
use shared_kernel::types::AuditReason;
use uuid::Uuid;

#[tokio::test]
async fn test_unsuspend_account_success() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();

    // 1. On crée un compte et on le suspend au niveau du domaine
    let account = f.builder()?.with_state(AccountState::SUSPENDED).build()?;

    let version_snapshot = account.version();
    f.account_repo().insert(account);

    let reason = AuditReason::try_new("Good behavior")?;
    let cmd = UnsuspendCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.account_id(), version_snapshot),
        region: f.region(),
        reason: reason.clone(),
    };

    // Act
    f.bus()
        .execute::<AccountCommandCtx, UnsuspendCommand, ()>(f.command_ctx().clone(), cmd)
        .await?;

    // Assert
    f.account_assertions()
        .assert_account_state(f.account_id(), |acc| {
            assert_eq!(*acc.identity().state(), AccountState::ACTIVE);
            assert_eq!(acc.version(), version_snapshot + 1);
        })
        .await;

    f.assert_outbox(1, Some(AccountEvent::UNSUSPENDED));

    Ok(())
}

#[tokio::test]
async fn test_unsuspend_technical_idempotency() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();
    let cmd_id = Uuid::new_v4();

    // On simule une interception transparente par la barrière d'idempotence technique
    f.idempotency_repo().save(None, &cmd_id).await?;

    let mut account = f.builder()?.build()?;
    account.suspend(AuditReason::try_new("Suspicious activity")?)?;
    account.pull_events();

    let version_snapshot = account.version();
    f.account_repo().insert(account);

    let cmd = UnsuspendCommand {
        command_id: cmd_id,
        target: CommandTarget::versioned(f.account_id(), version_snapshot),
        region: f.region(),
        reason: AuditReason::try_new("Duplicate call")?,
    };

    // Act
    let result = f
        .bus()
        .execute::<AccountCommandCtx, UnsuspendCommand, ()>(f.command_ctx().clone(), cmd)
        .await;

    // Assert
    assert!(
        result.is_ok(),
        "L'idempotence technique doit court-circuiter de façon transparente (Ok)"
    );

    // L'agrégat reste suspendu, aucun changement de version
    f.account_assertions()
        .assert_account_state(f.account_id(), |acc| {
            assert_eq!(*acc.identity().state(), AccountState::SUSPENDED);
            assert_eq!(acc.version(), version_snapshot);
        })
        .await;

    // L'outbox locale reste intacte et vierge d'événements dupliqués
    f.account_assertions()
        .assert_no_events_for(f.account_id())
        .await;

    Ok(())
}

#[tokio::test]
async fn test_unsuspend_business_idempotency() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();

    // Idempotence métier : Le compte est déjà actif (ou dans l'état configuré par défaut par le builder)
    let account = f.builder()?.with_state(AccountState::ACTIVE).build()?;
    let version_snapshot = account.version();
    f.account_repo().insert(account);

    let cmd = UnsuspendCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.account_id(), version_snapshot),
        region: f.region(),
        reason: AuditReason::try_new("Already good")?,
    };

    // Act
    f.bus()
        .execute::<AccountCommandCtx, UnsuspendCommand, ()>(f.command_ctx().clone(), cmd)
        .await?;

    // Assert
    f.account_assertions()
        .assert_account_state(f.account_id(), |acc| {
            assert_eq!(*acc.identity().state(), AccountState::ACTIVE);
            assert_eq!(acc.version(), version_snapshot);
        })
        .await;

    // Aucun événement produit puisque l'état est resté identique
    f.account_assertions()
        .assert_no_events_for(f.account_id())
        .await;

    Ok(())
}
