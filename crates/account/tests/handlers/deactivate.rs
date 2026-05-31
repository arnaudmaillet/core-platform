
use account::commands::lifecycle::DeactivateCommand;
use account::context::AccountCommandContext;
use account::events::AccountEvent;
use account::types::AccountState;
use account_test_utils::AccountTestFixture;
use shared_kernel::{
    command::CommandTarget,
    core::{ErrorCode, Result, Versioned},
};
use shared_kernel_test_utils::repositories::TransactionManagerStub;
use uuid::Uuid;

#[tokio::test]
async fn test_deactivate_account_success() -> Result<()> {
    let f = AccountTestFixture::new();

    // 1. Arrange : Compte initial actif
    let account = f.builder()?.build()?;
    let version_snapshot = account.version();
    f.account_repo().insert(account);

    let cmd = DeactivateCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::new(f.account_id(), f.region(), version_snapshot),
        reason: None,
    };

    // 2. Act
    f.bus()
        .execute::<AccountCommandContext<TransactionManagerStub>, DeactivateCommand, ()>(f.command_ctx().clone(), cmd)
        .await?;

    // 3. Assert
    f.assert_account(|acc| {
        assert_eq!(*acc.identity().state(), AccountState::DEACTIVATED);
        assert_eq!(acc.version(), version_snapshot + 1);
    })
    .await?;

    f.assert_outbox(1, Some(AccountEvent::DEACTIVATED));

    Ok(())
}

#[tokio::test]
async fn test_deactivate_technical_idempotency() -> Result<()> {
    let f = AccountTestFixture::new();
    let cmd_id = Uuid::new_v4();

    // Arrange : Simulation d'une commande de désactivation déjà enregistrée
    f.idempotency_repo().seed(cmd_id);

    let account = f.builder()?.build()?;
    let version_snapshot = account.version();
    f.account_repo().insert(account);

    let cmd = DeactivateCommand {
        command_id: cmd_id,
        target: CommandTarget::new(f.account_id(), f.region(), version_snapshot),
        reason: None,
    };

    // 2. Act
    let result = f
        .bus()
        .execute::<AccountCommandContext<TransactionManagerStub>, DeactivateCommand, ()>(f.command_ctx().clone(), cmd)
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
async fn test_deactivate_business_idempotency() -> Result<()> {
    let f = AccountTestFixture::new();

    // Arrange : Compte DÉJÀ désactivé
    let mut account = f.builder()?.build()?;
    account.deactivate(None)?;

    let version_snapshot = account.version();
    f.account_repo().insert(account);

    let cmd = DeactivateCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::new(f.account_id(), f.region(), version_snapshot),
        reason: None,
    };

    // 2. Act
    f.bus()
        .execute::<AccountCommandContext<TransactionManagerStub>, DeactivateCommand, ()>(f.command_ctx().clone(), cmd)
        .await?;

    // 3. Assert
    f.assert_account(|acc| {
        assert_eq!(*acc.identity().state(), AccountState::DEACTIVATED);
        assert_eq!(acc.version(), version_snapshot);
    })
    .await?;

    f.assert_outbox(0, None);

    Ok(())
}

#[tokio::test]
async fn test_deactivate_not_found() -> Result<()> {
    let f = AccountTestFixture::new();

    let cmd = DeactivateCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::new(f.account_id(), f.region(), 0),
        reason: None,
    };

    // Act
    let result = f
        .bus()
        .execute::<AccountCommandContext<TransactionManagerStub>, DeactivateCommand, ()>(f.command_ctx().clone(), cmd)
        .await;

    // Assert
    match result {
        Err(e) => {
            assert_eq!(e.code, ErrorCode::NotFound);
            assert!(e.message.contains("Account"));
        }
        Ok(_) => panic!("Should have failed: Account does not exist"),
    }
    f.assert_outbox(0, None);

    Ok(())
}
