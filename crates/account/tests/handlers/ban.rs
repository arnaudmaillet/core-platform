// crates/account/src/application/use_cases/moderation/ban/ban_use_case_test.rs

use account::commands::moderation::BanCommand;
use account::context::AccountCommandCtx;
use account::events::AccountEvent;
use account::types::AccountState;
use account_test_utils::asserts::AccountRepositoryAsserts;

use account_test_utils::AccountTestFixture;
use shared_kernel::{
    command::CommandTarget,
    core::{Error, ErrorCode, Result, Versioned},
    idempotency::IdempotencyRepository,
    messaging::EventEmitter,
    types::AuditReason,
};
use uuid::Uuid;

#[tokio::test]
async fn test_ban_account_success() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();

    // 1. On prépare un compte actif
    let account = f.builder()?.build()?;
    let version_snapshot = account.version();
    f.account_repo().insert(account);

    let reason = AuditReason::try_new("TOS Violation")?;
    let cmd = BanCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.account_id(), version_snapshot),
        region: f.region(),
        reason: reason.clone(),
    };

    // Act
    f.bus()
        .execute::<AccountCommandCtx, BanCommand, ()>(f.command_ctx().clone(), cmd)
        .await?;

    // Assert
    f.account_assertions()
        .assert_account_state(f.account_id(), |acc| {
            assert_eq!(*acc.identity().state(), AccountState::BANNED);
            assert_eq!(acc.version(), version_snapshot + 1);
        })
        .await;

    f.assert_outbox(1, Some(AccountEvent::BANNED));

    Ok(())
}

#[tokio::test]
async fn test_ban_technical_idempotency() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();
    let cmd_id = Uuid::new_v4();

    // On simule une commande déjà interceptée par la barrière d'idempotence technique
    f.idempotency_repo().save(None, &cmd_id).await?;

    let account = f.builder()?.build()?;
    let version_snapshot = account.version();
    f.account_repo().insert(account);

    let cmd = BanCommand {
        command_id: cmd_id,
        target: CommandTarget::versioned(f.account_id(), version_snapshot),
        region: f.region(),
        reason: AuditReason::try_new("Duplicate Ban")?,
    };

    // Act
    let result = f
        .bus()
        .execute::<AccountCommandCtx, BanCommand, ()>(f.command_ctx().clone(), cmd)
        .await;

    // Assert
    assert!(
        result.is_ok(),
        "L'idempotence technique doit court-circuiter de façon transparente (Ok)"
    );

    // L'outbox locale reste vide
    f.account_assertions()
        .assert_no_events_for(f.account_id())
        .await;

    Ok(())
}

#[tokio::test]
async fn test_ban_business_idempotency() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();

    // Compte déjà Banni en amont
    let mut account = f.builder()?.build()?;
    account.ban(AuditReason::try_new("First reason")?)?;
    account.pull_events(); // On vide l'outbox du setup

    let version_snapshot = account.version();
    f.account_repo().insert(account);

    let cmd = BanCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.account_id(), version_snapshot),
        region: f.region(),
        reason: AuditReason::try_new("Second attempt")?,
    };

    // Act
    f.bus()
        .execute::<AccountCommandCtx, BanCommand, ()>(f.command_ctx().clone(), cmd)
        .await?;

    // Assert
    // Pas de modification d'état, pas d'incrément de version
    f.account_assertions()
        .assert_account_state(f.account_id(), |acc| {
            assert_eq!(*acc.identity().state(), AccountState::BANNED);
            assert_eq!(acc.version(), version_snapshot);
        })
        .await;

    // L'invariant métier bloque l'émission de nouveaux événements
    f.account_assertions()
        .assert_no_events_for(f.account_id())
        .await;

    Ok(())
}

#[tokio::test]
async fn test_worst_case_account_not_found() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();

    let cmd = BanCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.account_id(), 0),
        region: f.region(),
        reason: AuditReason::try_new("Violating TOS")?,
    };

    // Act
    let result = f
        .bus()
        .execute::<AccountCommandCtx, BanCommand, ()>(f.command_ctx().clone(), cmd)
        .await;

    // Assert
    match result {
        Err(e) => {
            assert_eq!(e.code, ErrorCode::NotFound);
            assert!(e.message.contains("Account"));
        }
        Ok(_) => panic!("Le cas d'usage aurait dû échouer : le compte n'existe pas"),
    }

    // Aucun événement produit
    f.account_assertions()
        .assert_no_events_for(f.account_id())
        .await;

    Ok(())
}

#[tokio::test]
async fn test_concurrency_retry_success() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();
    let account = f.builder()?.build()?;
    let version_snapshot = account.version();
    f.account_repo().insert(account);

    // On simule une erreur de concurrence transitoire (OCC conflict) qui disparaît au retry
    f.account_repo().set_error_once(Error::concurrency_conflict(
        "Race condition / Concurrency conflict",
    ));

    let cmd = BanCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.account_id(), version_snapshot),
        region: f.region(),
        reason: AuditReason::try_new("System ban")?,
    };

    // Act
    let result = f
        .bus()
        .execute::<AccountCommandCtx, BanCommand, ()>(f.command_ctx().clone(), cmd)
        .await;

    // Assert
    assert!(
        result.is_ok(),
        "Le bus aurait dû absorber le conflit initial et réussir après retry"
    );

    f.account_assertions()
        .assert_account_state(f.account_id(), |acc| {
            assert_eq!(*acc.identity().state(), AccountState::BANNED);
        })
        .await;

    Ok(())
}

#[tokio::test]
async fn test_worst_case_atomic_outbox_failure() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();

    let account = f.builder()?.build()?;
    let version_snapshot = account.version();
    f.account_repo().insert(account);

    // On simule une erreur bloquante lors du commit transactionnel de l'outbox (ex: partition pleine)
    f.outbox_repo().set_error(Error::internal("Disk full"));

    let cmd = BanCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.account_id(), version_snapshot),
        region: f.region(),
        reason: AuditReason::try_new("System ban")?,
    };

    // Act
    let result = f
        .bus()
        .execute::<AccountCommandCtx, BanCommand, ()>(f.command_ctx().clone(), cmd)
        .await;

    // Assert
    match result {
        Err(e) => {
            assert_eq!(e.code, ErrorCode::InternalError);
        }
        Ok(_) => panic!("La transaction globale aurait dû Rollback suite au crash Outbox"),
    }

    // Le stub imite le comportement transactionnel : la modification sur l'agrégat n'est pas persistée
    // car check_error a intercepté l'échec d'infrastructure avant le commit final.
    Ok(())
}
