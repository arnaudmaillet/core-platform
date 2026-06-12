// crates/account/src/application/use_cases/moderation/unban/unban_use_case_test.rs

use account::events::AccountEvent;
use account::types::AccountState;
use account::{commands::moderation::UnbanCommand, context::AccountCommandCtx};
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
async fn test_unban_account_success() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();

    // 1. On prépare un compte initialement BANNI pour pouvoir le débannir
    let mut account = f.builder()?.build()?;
    account.ban(AuditReason::try_new("Initial violation")?)?;
    account.pull_events(); // On purge l'événement de ban du setup

    let version_snapshot = account.version();
    let target_account_id = f.account_id();
    f.account_repo().insert(account);

    let reason = AuditReason::try_new("Appeal accepted")?;
    let cmd = UnbanCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(target_account_id, version_snapshot),
        region: f.region(),
        reason: reason.clone(),
    };

    // Act
    f.bus()
        .execute::<AccountCommandCtx, UnbanCommand, ()>(f.command_ctx().clone(), cmd)
        .await?;

    // Assert
    // Le compte doit repasser à l'état ACTIVE
    f.account_assertions()
        .assert_account_state(target_account_id, |acc| {
            assert_eq!(*acc.identity().state(), AccountState::ACTIVE);
            assert_eq!(acc.version(), version_snapshot + 1);
        })
        .await;

    f.assert_outbox(1, Some(AccountEvent::UNBANNED));

    Ok(())
}

#[tokio::test]
async fn test_unban_technical_idempotency() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();
    let cmd_id = Uuid::new_v4();

    // On simule une commande déjà interceptée par la barrière d'idempotence technique
    f.idempotency_repo().save(None, &cmd_id).await?;

    let mut account = f.builder()?.build()?;
    account.ban(AuditReason::try_new("Initial violation")?)?;
    account.pull_events();

    let version_snapshot = account.version();
    let target_account_id = f.account_id();
    f.account_repo().insert(account);

    let cmd = UnbanCommand {
        command_id: cmd_id,
        target: CommandTarget::versioned(target_account_id, version_snapshot),
        region: f.region(),
        reason: AuditReason::try_new("Duplicate network call")?,
    };

    // Act
    let result = f
        .bus()
        .execute::<AccountCommandCtx, UnbanCommand, ()>(f.command_ctx().clone(), cmd)
        .await;

    // Assert
    assert!(
        result.is_ok(),
        "L'idempotence technique doit court-circuiter de façon transparente (Ok)"
    );

    // L'agrégat n'a pas bougé et reste BANNED puisque la commande a été ignorée
    f.account_assertions()
        .assert_account_state(target_account_id, |acc| {
            assert_eq!(*acc.identity().state(), AccountState::BANNED);
            assert_eq!(acc.version(), version_snapshot);
        })
        .await;

    // Aucun événement n'est ré-émis
    f.account_assertions()
        .assert_no_events_for(target_account_id)
        .await;

    Ok(())
}

#[tokio::test]
async fn test_unban_business_idempotency() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();

    // Idempotence métier : Le compte est déjà actif (non banni)
    let account = f.builder()?.with_state(AccountState::ACTIVE).build()?;
    let version_snapshot = account.version();
    let target_account_id = f.account_id();
    f.account_repo().insert(account);

    let cmd = UnbanCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(target_account_id, version_snapshot),
        region: f.region(),
        reason: AuditReason::try_new("Accidental secondary unban call")?,
    };

    // Act
    f.bus()
        .execute::<AccountCommandCtx, UnbanCommand, ()>(f.command_ctx().clone(), cmd)
        .await?;

    // Assert
    // Pas de modification d'état, pas d'incrément de version
    f.account_assertions()
        .assert_account_state(target_account_id, |acc| {
            assert_eq!(*acc.identity().state(), AccountState::ACTIVE);
            assert_eq!(acc.version(), version_snapshot);
        })
        .await;

    // Aucun événement produit puisque l'état n'a pas changé
    f.account_assertions()
        .assert_no_events_for(target_account_id)
        .await;

    Ok(())
}

#[tokio::test]
async fn test_worst_case_account_not_found() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();
    let target_account_id = f.account_id();

    let cmd = UnbanCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(target_account_id, 0),
        region: f.region(),
        reason: AuditReason::try_new("Unban non-existent")?,
    };

    // Act
    let result = f
        .bus()
        .execute::<AccountCommandCtx, UnbanCommand, ()>(f.command_ctx().clone(), cmd)
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
        .assert_no_events_for(target_account_id)
        .await;

    Ok(())
}

#[tokio::test]
async fn test_concurrency_retry_success() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();

    let mut account = f.builder()?.build()?;
    account.ban(AuditReason::try_new("Initial violation")?)?;
    account.pull_events();

    let version_snapshot = account.version();
    let target_account_id = f.account_id();
    f.account_repo().insert(account);

    // Erreur OCC transitoire
    f.account_repo().set_error_once(Error::concurrency_conflict(
        "Race condition / Concurrency conflict",
    ));

    let cmd = UnbanCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(target_account_id, version_snapshot),
        region: f.region(),
        reason: AuditReason::try_new("Appeal processed")?,
    };

    // Act
    let result = f
        .bus()
        .execute::<AccountCommandCtx, UnbanCommand, ()>(f.command_ctx().clone(), cmd)
        .await;

    // Assert
    assert!(
        result.is_ok(),
        "Le bus aurait dû absorber le conflit initial et réussir après retry"
    );

    f.account_assertions()
        .assert_account_state(target_account_id, |acc| {
            assert_eq!(*acc.identity().state(), AccountState::ACTIVE);
        })
        .await;

    Ok(())
}

#[tokio::test]
async fn test_worst_case_atomic_outbox_failure() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();

    let mut account = f.builder()?.build()?;
    account.ban(AuditReason::try_new("Initial violation")?)?;
    account.pull_events();

    let version_snapshot = account.version();
    let target_account_id = f.account_id();
    f.account_repo().insert(account);

    // Panne d'outbox atomique
    f.outbox_repo().set_error(Error::internal("Disk full"));

    let cmd = UnbanCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(target_account_id, version_snapshot),
        region: f.region(),
        reason: AuditReason::try_new("Failed appeal unban process")?,
    };

    // Act
    let result = f
        .bus()
        .execute::<AccountCommandCtx, UnbanCommand, ()>(f.command_ctx().clone(), cmd)
        .await;

    // Assert
    match result {
        Err(e) => {
            assert_eq!(e.code, ErrorCode::InternalError);
        }
        Ok(_) => panic!("La transaction globale aurait dû Rollback suite au crash Outbox"),
    }

    Ok(())
}
