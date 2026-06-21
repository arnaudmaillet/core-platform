// crates/account/src/application/use_cases/lifecycle/deactivate/deactivate_use_case_test.rs

use account_old::commands::lifecycle::DeactivateCommand;
use account_old::context::AccountCommandCtx;
use account_old::events::AccountEvent;
use account_old::types::AccountState;
use account_test_utils::asserts::AccountRepositoryAsserts;

use account_test_utils::AccountTestFixture;
use shared_kernel::{
    command::CommandTarget,
    core::{ErrorCode, Result, Versioned},
    idempotency::IdempotencyRepository,
};
use uuid::Uuid;

#[tokio::test]
async fn test_deactivate_account_success() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();

    // 1. On prépare un compte initial actif
    let account = f.builder()?.build()?;
    let version_snapshot = account.version();
    f.account_repo().insert(account);

    let cmd = DeactivateCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.account_id(), version_snapshot),
        region: f.server_region(),
        reason: None,
    };

    // Act
    f.bus()
        .execute::<AccountCommandCtx, DeactivateCommand, ()>(f.command_ctx().clone(), cmd)
        .await?;

    // Assert
    f.account_assertions()
        .assert_account_state(f.account_id(), |acc| {
            assert_eq!(*acc.identity().state(), AccountState::DEACTIVATED);
            assert_eq!(acc.version(), version_snapshot + 1);
        })
        .await;

    f.assert_outbox(1, Some(AccountEvent::DEACTIVATED));

    Ok(())
}

#[tokio::test]
async fn test_deactivate_technical_idempotency() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();
    let cmd_id = Uuid::new_v4();

    // On simule une commande déjà enregistrée dans le premier rideau d'idempotence
    f.idempotency_repo().save(None, &cmd_id).await?;

    let account = f.builder()?.build()?;
    let version_snapshot = account.version();
    f.account_repo().insert(account);

    let cmd = DeactivateCommand {
        command_id: cmd_id,
        target: CommandTarget::versioned(f.account_id(), version_snapshot),
        region: f.server_region(),
        reason: None,
    };

    // Act
    let result = f
        .bus()
        .execute::<AccountCommandCtx, DeactivateCommand, ()>(f.command_ctx().clone(), cmd)
        .await;

    // Assert
    assert!(
        result.is_ok(),
        "L'idempotence technique doit court-circuiter de façon transparente (Ok)"
    );

    // L'agrégat en base n'a pas subi de double mutation
    f.account_assertions()
        .assert_account_state(f.account_id(), |acc| {
            assert_eq!(*acc.identity().state(), AccountState::UNVERIFIED);
            assert_eq!(acc.version(), version_snapshot);
        })
        .await;

    // Pas de duplication d'événements dans l'outbox locale
    f.account_assertions()
        .assert_no_events_for(f.account_id())
        .await;

    Ok(())
}

#[tokio::test]
async fn test_deactivate_business_idempotency() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();

    // Compte déjà désactivé au niveau du domaine (Idempotence métier)
    let account = f.builder()?.with_state(AccountState::DEACTIVATED).build()?;

    let version_snapshot = account.version();
    f.account_repo().insert(account);

    let cmd = DeactivateCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.account_id(), version_snapshot),
        region: f.server_region(),
        reason: None,
    };

    // Act
    f.bus()
        .execute::<AccountCommandCtx, DeactivateCommand, ()>(f.command_ctx().clone(), cmd)
        .await?;

    // Assert
    // Pas de modification d'état, pas d'incrément de version (No-Op transactionnel)
    f.account_assertions()
        .assert_account_state(f.account_id(), |acc| {
            assert_eq!(*acc.identity().state(), AccountState::DEACTIVATED);
            assert_eq!(acc.version(), version_snapshot);
        })
        .await;

    // Aucun événement produit puisque l'état est inchangé
    f.account_assertions()
        .assert_no_events_for(f.account_id())
        .await;

    Ok(())
}

#[tokio::test]
async fn test_deactivate_not_found() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();

    let cmd = DeactivateCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.account_id(), 0),
        region: f.server_region(),
        reason: None,
    };

    // Act
    let result = f
        .bus()
        .execute::<AccountCommandCtx, DeactivateCommand, ()>(f.command_ctx().clone(), cmd)
        .await;

    // Assert
    match result {
        Err(e) => {
            assert_eq!(e.code, ErrorCode::NotFound);
            assert!(e.message.contains("Account"));
        }
        Ok(_) => panic!("Le cas d'usage aurait dû échouer : le compte n'existe pas"),
    }

    f.account_assertions()
        .assert_no_events_for(f.account_id())
        .await;

    Ok(())
}
