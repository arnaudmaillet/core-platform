// crates/account/src/application/use_cases/lifecycle/activate/activate_use_case_test.rs

use account::commands::lifecycle::ActivateCommand;
use account::context::AccountCommandContext;
use account::events::AccountEvent;
use account::types::AccountState;
use account_test_utils::AccountTestFixture;
use shared_kernel::command::CommandTarget;
use shared_kernel::core::{ErrorCode, Result, Versioned};
use shared_kernel::messaging::EventEmitter;
use shared_kernel::types::AuditReason;
use shared_kernel_test_utils::repositories::TransactionManagerStub;
use uuid::Uuid;

#[tokio::test]
async fn test_activate_account_success() -> Result<()> {
    let f = AccountTestFixture::new();

    // 1. Arrange : On crée un compte désactivé
    let mut account = f.builder()?.build()?;

    account.deactivate(None)?;
    account.pull_events();

    let version_snapshot = account.version();

    f.account_repo().insert(account);

    let cmd = ActivateCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.account_id(), f.region(), version_snapshot),
    };

    f.bus()
        .execute::<AccountCommandContext<TransactionManagerStub>, ActivateCommand, ()>(
            f.command_ctx().clone(),
            cmd,
        )
        .await?;

    f.assert_account(|acc| {
        assert_eq!(*acc.identity().state(), AccountState::ACTIVE);
        assert_eq!(acc.version(), version_snapshot + 1);
    })
    .await?;

    // 5. Outbox
    f.assert_outbox(1, Some(AccountEvent::ACTIVATED));

    Ok(())
}

#[tokio::test]
async fn test_activate_technical_idempotency() -> Result<()> {
    let f = AccountTestFixture::new();
    let cmd_id = Uuid::new_v4();

    // Arrange : On simule une commande déjà traitée
    f.idempotency_repo().seed(cmd_id);

    let mut account = f.builder()?.build()?;
    account.deactivate(None)?;

    let version_snapshot = account.version();
    f.account_repo().insert(account);

    let cmd = ActivateCommand {
        command_id: cmd_id,
        target: CommandTarget::versioned(f.account_id(), f.region(), version_snapshot),
    };

    // 2. Act
    let result = f
        .bus()
        .execute::<AccountCommandContext<TransactionManagerStub>, ActivateCommand, ()>(
            f.command_ctx().clone(),
            cmd,
        )
        .await;

    // 3. Assert
    assert!(
        result.is_ok(),
        "L'idempotence technique doit être transparente (Ok)"
    );

    f.assert_account(|acc| {
        assert_eq!(*acc.identity().state(), AccountState::DEACTIVATED);
        assert_eq!(acc.version(), version_snapshot);
    })
    .await?;

    f.assert_outbox(0, None);

    Ok(())
}

#[tokio::test]
async fn test_activate_business_idempotency() -> Result<()> {
    let f = AccountTestFixture::new();

    // Arrange : Compte déjà actif
    let mut account = f.builder()?.build()?;
    account.activate()?;

    let version_snapshot = account.version();
    f.account_repo().insert(account);

    let cmd = ActivateCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.account_id(), f.region(), version_snapshot),
    };

    // 2. Act
    f.bus()
        .execute::<AccountCommandContext<TransactionManagerStub>, ActivateCommand, ()>(
            f.command_ctx().clone(),
            cmd,
        )
        .await?;

    // 3. Assert
    f.assert_account(|acc| {
        assert_eq!(*acc.identity().state(), AccountState::ACTIVE);
        assert_eq!(acc.version(), version_snapshot);
    })
    .await?;

    f.assert_outbox(0, None);

    Ok(())
}

#[tokio::test]
async fn test_activate_forbidden_if_banned() -> Result<()> {
    let f = AccountTestFixture::new();

    // Arrange
    let mut account = f.builder()?.build()?;
    account.ban(AuditReason::try_new("Violation")?)?;
    let version_snapshot = account.version();
    f.account_repo().insert(account);

    let cmd = ActivateCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.account_id(), f.region(), version_snapshot),
    };

    // 2. Act
    let result = f
        .bus()
        .execute::<AccountCommandContext<TransactionManagerStub>, ActivateCommand, ()>(
            f.command_ctx().clone(),
            cmd,
        )
        .await;

    // 3. Assert
    match result {
        Err(e) => {
            assert_eq!(e.code, ErrorCode::Forbidden);
        }
        Ok(_) => panic!("Should have failed: a banned account cannot change its email"),
    }

    f.assert_account(|acc| {
        assert_eq!(*acc.identity().state(), AccountState::BANNED);
        assert_eq!(acc.version(), version_snapshot);
    })
    .await?;

    f.assert_outbox(0, None);

    Ok(())
}
