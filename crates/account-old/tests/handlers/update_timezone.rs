// crates/account/src/application/use_cases/settings/update_timezone/update_timezone_use_case_test.rs

use account::commands::settings::UpdateTimezoneCommand;
use account::context::AccountCommandCtx;
use account::entities::AccountSettingsBuilder;
use account::events::AccountEvent;
use account::types::AccountState;
use account_test_utils::asserts::AccountRepositoryAsserts;

use account_test_utils::AccountTestFixture;
use shared_kernel::command::CommandTarget;
use shared_kernel::core::{Result, Versioned};
use shared_kernel::geo::Timezone;
use shared_kernel::idempotency::IdempotencyRepository;
use uuid::Uuid;

#[tokio::test]
async fn test_update_timezone_success() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();
    let initial_tz = Timezone::from_raw("UTC");
    let new_tz = Timezone::from_raw("Europe/Paris");

    // 1. Compte actif configuré initialement en UTC
    let account = f
        .builder()?
        .with_state(AccountState::ACTIVE)
        .settings(|s: AccountSettingsBuilder| s.with_timezone(initial_tz))
        .build()?;

    let version_snapshot = account.version();
    f.account_repo().insert(account);

    let cmd = UpdateTimezoneCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.account_id(), version_snapshot),
        region: f.server_region(),
        new_timezone: new_tz.clone(),
    };

    // Act
    f.bus()
        .execute::<AccountCommandCtx, UpdateTimezoneCommand, ()>(f.command_ctx().clone(), cmd)
        .await?;

    // Assert
    f.account_assertions()
        .assert_account_state(f.account_id(), |acc| {
            assert_eq!(acc.settings().timezone(), &new_tz);
            assert_eq!(acc.version(), version_snapshot + 1);
        })
        .await;

    f.assert_outbox(1, Some(AccountEvent::TIMEZONE_UPDATED));

    Ok(())
}

#[tokio::test]
async fn test_update_timezone_technical_idempotency() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();
    let cmd_id = Uuid::new_v4();
    let requested_tz = Timezone::from_raw("Europe/Paris");

    // On simule une commande déjà traitée techniquement interceptée au premier rideau
    f.idempotency_repo().save(None, &cmd_id).await?;

    let account = f.builder()?.with_state(AccountState::ACTIVE).build()?;
    let version_snapshot = account.version();
    f.account_repo().insert(account);

    let cmd = UpdateTimezoneCommand {
        command_id: cmd_id,
        target: CommandTarget::versioned(f.account_id(), version_snapshot),
        region: f.server_region(),
        new_timezone: requested_tz.clone(),
    };

    // Act
    let result = f
        .bus()
        .execute::<AccountCommandCtx, UpdateTimezoneCommand, ()>(f.command_ctx().clone(), cmd)
        .await;

    // Assert
    assert!(
        result.is_ok(),
        "L'idempotence technique doit court-circuiter de façon transparente (Ok)"
    );

    // L'agrégat en base n'a subi aucune mutation
    f.account_assertions()
        .assert_account_state(f.account_id(), |acc| {
            assert_ne!(acc.settings().timezone(), &requested_tz);
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
async fn test_update_timezone_business_idempotency() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();
    let current_tz = Timezone::from_raw("Europe/Paris");

    // Idempotence métier : Le compte possède déjà exactement la même timezone
    let account = f
        .builder()?
        .with_state(AccountState::ACTIVE)
        .settings(|s: AccountSettingsBuilder| s.with_timezone(current_tz.clone()))
        .build()?;

    let version_snapshot = account.version();
    f.account_repo().insert(account);

    let cmd = UpdateTimezoneCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.account_id(), version_snapshot),
        region: f.server_region(),
        new_timezone: current_tz,
    };

    // Act
    f.bus()
        .execute::<AccountCommandCtx, UpdateTimezoneCommand, ()>(f.command_ctx().clone(), cmd)
        .await?;

    // Assert
    // L'exécution réussit mais ne produit aucune mutation (No-Op transactionnel)
    f.account_assertions()
        .assert_account_state(f.account_id(), |acc| {
            assert_eq!(
                acc.version(),
                version_snapshot,
                "La version ne doit pas bouger si l'état était déjà identique"
            );
        })
        .await;

    // Aucun événement produit puisque l'état est resté identique
    f.account_assertions()
        .assert_no_events_for(f.account_id())
        .await;

    Ok(())
}
