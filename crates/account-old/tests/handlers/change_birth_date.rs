// crates/account/src/application/use_cases/settings/change_birth_date/change_birth_date_use_case_test.rs

use account_old::commands::settings::ChangeBirthDateCommand;
use account_old::context::AccountCommandCtx;
use account_old::events::AccountEvent;
use account_old::types::{AccountState, BirthDate};
use account_test_utils::asserts::AccountRepositoryAsserts;

use account_test_utils::AccountTestFixture;
use chrono::NaiveDate;
use shared_kernel::command::CommandTarget;
use shared_kernel::core::{Error, ErrorCode, Result, Versioned};
use shared_kernel::idempotency::IdempotencyRepository;
use uuid::Uuid;

fn adult_birth_date() -> BirthDate {
    BirthDate::try_new(NaiveDate::from_ymd_opt(2000, 1, 1).unwrap()).unwrap()
}

#[tokio::test]
async fn test_change_birth_date_success() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();
    let account = f.builder()?.with_state(AccountState::ACTIVE).build()?;
    let version_snapshot = account.version();
    f.account_repo().insert(account);

    let new_date = adult_birth_date();
    let cmd = ChangeBirthDateCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.account_id(), version_snapshot),
        region: f.server_region(),
        new_birth_date: new_date.clone(),
    };

    // Act
    f.bus()
        .execute::<AccountCommandCtx, ChangeBirthDateCommand, ()>(f.command_ctx().clone(), cmd)
        .await?;

    // Assert
    f.account_assertions()
        .assert_account_state(f.account_id(), |acc| {
            assert_eq!(acc.identity().birth_date(), Some(&new_date));
            assert_eq!(acc.version(), version_snapshot + 1);
        })
        .await;

    f.assert_outbox(1, Some(AccountEvent::BIRTH_DATE_CHANGED));

    Ok(())
}

#[tokio::test]
async fn test_change_birth_date_technical_idempotency() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();
    let cmd_id = Uuid::new_v4();

    // On simule une commande déjà traitée techniquement interceptée au premier rideau
    f.idempotency_repo().save(None, &cmd_id).await?;

    let account = f.builder()?.with_state(AccountState::ACTIVE).build()?;
    let version_snapshot = account.version();
    f.account_repo().insert(account);

    let new_date = adult_birth_date();
    let cmd = ChangeBirthDateCommand {
        command_id: cmd_id,
        target: CommandTarget::versioned(f.account_id(), version_snapshot),
        region: f.server_region(),
        new_birth_date: new_date.clone(),
    };

    // Act
    let result = f
        .bus()
        .execute::<AccountCommandCtx, ChangeBirthDateCommand, ()>(f.command_ctx().clone(), cmd)
        .await;

    // Assert
    assert!(
        result.is_ok(),
        "L'idempotence technique doit court-circuiter de façon transparente (Ok)"
    );

    // L'agrégat n'a subi aucune mutation interne
    f.account_assertions()
        .assert_account_state(f.account_id(), |acc| {
            assert_ne!(acc.identity().birth_date(), Some(&new_date));
            assert_eq!(acc.version(), version_snapshot);
        })
        .await;

    // Aucun événement n'est ré-émis ou dupliqué dans l'outbox locale
    f.account_assertions()
        .assert_no_events_for(f.account_id())
        .await;

    Ok(())
}

#[tokio::test]
async fn test_change_birth_date_forbidden_when_restricted() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();
    let account = f.builder()?.with_state(AccountState::BANNED).build()?;
    let version_snapshot = account.version();
    f.account_repo().insert(account);

    let new_date = adult_birth_date();
    let cmd = ChangeBirthDateCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.account_id(), version_snapshot),
        region: f.server_region(),
        new_birth_date: new_date.clone(),
    };

    // Act
    let result = f
        .bus()
        .execute::<AccountCommandCtx, ChangeBirthDateCommand, ()>(f.command_ctx().clone(), cmd)
        .await;

    // Assert
    match result {
        Err(e) => {
            assert_eq!(e.code, ErrorCode::Forbidden);
        }
        Ok(_) => panic!(
            "Le cas d'usage aurait dû échouer : un compte banni ne peut pas modifier ses settings"
        ),
    }

    // Sécurité de l'état : l'invariant a tenu bon, aucune écriture
    f.account_assertions()
        .assert_account_state(f.account_id(), |acc| {
            assert_ne!(acc.identity().birth_date(), Some(&new_date));
            assert_eq!(acc.version(), version_snapshot);
        })
        .await;

    f.account_assertions()
        .assert_no_events_for(f.account_id())
        .await;

    Ok(())
}

#[tokio::test]
async fn test_change_birth_date_succeeds_after_retry() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();
    let account = f.builder()?.with_state(AccountState::ACTIVE).build()?;
    let version_snapshot = account.version();
    f.account_repo().insert(account);

    // Simulation d'une erreur de concurrence transitoire (Optimistic Lock Failure)
    f.account_repo().set_error_once(Error::concurrency_conflict(
        "Optimistic lock failure / OCC Concurrency conflict",
    ));

    let new_date = adult_birth_date();
    let cmd = ChangeBirthDateCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.account_id(), version_snapshot),
        region: f.server_region(),
        new_birth_date: new_date.clone(),
    };

    // Act : Le bus doit absorber le conflit initial et réussir au second essai grâce au retry automatique
    let result = f
        .bus()
        .execute::<AccountCommandCtx, ChangeBirthDateCommand, ()>(f.command_ctx().clone(), cmd)
        .await;

    // Assert
    assert!(
        result.is_ok(),
        "Le bus aurait dû retenter automatiquement et réussir"
    );

    f.account_assertions()
        .assert_account_state(f.account_id(), |acc| {
            assert_eq!(acc.identity().birth_date(), Some(&new_date));
            assert_eq!(acc.version(), version_snapshot + 1);
        })
        .await;

    // L'événement final est bien présent dans l'outbox après l'exécution résiliente
    f.assert_outbox(1, Some(AccountEvent::BIRTH_DATE_CHANGED));

    Ok(())
}
