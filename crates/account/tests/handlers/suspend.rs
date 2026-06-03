use account::commands::lifecycle::SuspendCommand;
use account::context::AccountCommandContext;
use account::events::AccountEvent;
use account::types::AccountState;
use account_test_utils::AccountTestFixture;
use shared_kernel::command::CommandTarget;
use shared_kernel::core::{Result, Versioned};
use shared_kernel::types::AuditReason;
use shared_kernel_test_utils::repositories::TransactionManagerStub;
use uuid::Uuid;

#[tokio::test]
async fn test_suspend_account_success() -> Result<()> {
    let f = AccountTestFixture::new();

    // 1. Arrange
    let account = f.builder()?.build()?;
    let version_snapshot = account.version();
    f.account_repo().insert(account);

    let cmd = SuspendCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.account_id(), f.region(), version_snapshot),
        reason: AuditReason::try_new("Under investigation for fraud")?,
    };

    // 2. Act
    f.bus()
        .execute::<AccountCommandContext<TransactionManagerStub>, SuspendCommand, ()>(
            f.command_ctx().clone(),
            cmd,
        )
        .await?;

    // 3. Assert
    f.assert_account(|acc| {
        assert_eq!(*acc.identity().state(), AccountState::SUSPENDED);
        assert_eq!(acc.version(), version_snapshot + 1);
    })
    .await?;

    f.assert_outbox(1, Some(AccountEvent::SUSPENDED));

    Ok(())
}

#[tokio::test]
async fn test_suspend_technical_idempotency() -> Result<()> {
    let f = AccountTestFixture::new();
    let cmd_id = Uuid::new_v4();

    // Arrange : Commande déjà enregistrée
    f.idempotency_repo().seed(cmd_id);

    let account = f.builder()?.build()?;
    let version_snapshot = account.version();
    f.account_repo().insert(account);

    let cmd = SuspendCommand {
        command_id: cmd_id,
        target: CommandTarget::versioned(f.account_id(), f.region(), version_snapshot),
        reason: AuditReason::try_new("Duplicate call")?,
    };

    // 2. Act
    let result = f
        .bus()
        .execute::<AccountCommandContext<TransactionManagerStub>, SuspendCommand, ()>(
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
        assert_eq!(*acc.identity().state(), AccountState::UNVERIFIED);
        assert_eq!(acc.version(), version_snapshot);
    })
    .await?;

    f.assert_outbox(0, None);

    Ok(())
}

#[tokio::test]
async fn test_suspend_business_idempotency() -> Result<()> {
    let f = AccountTestFixture::new();

    // Arrange : Compte déjà suspendu
    let mut account = f.builder()?.build()?;
    account.suspend(AuditReason::try_new("Original reason")?)?;

    let version_snapshot = account.version();
    f.account_repo().insert(account);

    let cmd = SuspendCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.account_id(), f.region(), version_snapshot),
        reason: AuditReason::try_new("Second call")?,
    };

    // 2. Act
    f.bus()
        .execute::<AccountCommandContext<TransactionManagerStub>, SuspendCommand, ()>(
            f.command_ctx().clone(),
            cmd,
        )
        .await?;

    // 3. Assert
    f.assert_account(|acc| {
        assert_eq!(*acc.identity().state(), AccountState::SUSPENDED);
        assert_eq!(acc.version(), version_snapshot);
    })
    .await?;

    f.assert_outbox(0, None);

    Ok(())
}
