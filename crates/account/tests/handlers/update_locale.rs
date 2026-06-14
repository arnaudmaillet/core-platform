// crates/account/src/application/use_cases/settings/update_locale/update_locale_use_case_test.rs

use account::commands::settings::UpdateLocaleCommand;
use account::context::AccountCommandCtx;
use account::events::AccountEvent;
use account::types::{AccountState, Locale};
use account_test_utils::asserts::AccountRepositoryAsserts;

use account_test_utils::AccountTestFixture;
use shared_kernel::command::CommandTarget;
use shared_kernel::core::{Result, Versioned};
use shared_kernel::idempotency::IdempotencyRepository;
use uuid::Uuid;

#[tokio::test]
async fn test_update_locale_success() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();
    let old_locale = Locale::from_raw("fr");
    let new_locale = Locale::from_raw("en");

    // 1. On prépare un compte actif avec une locale spécifique
    let account = f
        .builder()?
        .with_state(AccountState::ACTIVE)
        .with_locale(old_locale)
        .build()?;

    let version_snapshot = account.version();
    f.account_repo().insert(account);

    let cmd = UpdateLocaleCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.account_id(), version_snapshot),
        region: f.server_region(),
        new_locale: new_locale.clone(),
    };

    // Act
    f.bus()
        .execute::<AccountCommandCtx, UpdateLocaleCommand, ()>(f.command_ctx().clone(), cmd)
        .await?;

    // Assert
    f.account_assertions()
        .assert_account_state(f.account_id(), |acc| {
            assert_eq!(acc.identity().locale(), &new_locale);
            assert_eq!(acc.version(), version_snapshot + 1);
        })
        .await;

    f.assert_outbox(1, Some(AccountEvent::LOCALE_UPDATED));

    Ok(())
}

#[tokio::test]
async fn test_update_locale_technical_idempotency() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();
    let cmd_id = Uuid::new_v4();
    let requested_locale = Locale::from_raw("it");

    // On simule une commande déjà enregistrée interceptée au premier rideau d'idempotence technique
    f.idempotency_repo().save(None, &cmd_id).await?;

    let account = f.builder()?.with_state(AccountState::ACTIVE).build()?;
    let version_snapshot = account.version();
    f.account_repo().insert(account);

    let cmd = UpdateLocaleCommand {
        command_id: cmd_id,
        target: CommandTarget::versioned(f.account_id(), version_snapshot),
        region: f.server_region(),
        new_locale: requested_locale.clone(),
    };

    // Act
    let result = f
        .bus()
        .execute::<AccountCommandCtx, UpdateLocaleCommand, ()>(f.command_ctx().clone(), cmd)
        .await;

    // Assert
    assert!(
        result.is_ok(),
        "L'idempotence technique doit court-circuiter de façon transparente (Ok)"
    );

    // L'agrégat en base n'a subi aucun changement d'état ou de version
    f.account_assertions()
        .assert_account_state(f.account_id(), |acc| {
            assert_ne!(acc.identity().locale(), &requested_locale);
            assert_eq!(acc.version(), version_snapshot);
        })
        .await;

    // Aucun événement dupliqué n'est poussé dans l'outbox locale
    f.account_assertions()
        .assert_no_events_for(f.account_id())
        .await;

    Ok(())
}

#[tokio::test]
async fn test_update_locale_business_idempotency() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();
    let current_locale = Locale::from_raw("de");

    // Idempotence métier : le compte possède déjà la locale demandée
    let account = f
        .builder()?
        .with_state(AccountState::ACTIVE)
        .with_locale(current_locale.clone())
        .build()?;

    let version_snapshot = account.version();
    f.account_repo().insert(account);

    let cmd = UpdateLocaleCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.account_id(), version_snapshot),
        region: f.server_region(),
        new_locale: current_locale,
    };

    // Act
    f.bus()
        .execute::<AccountCommandCtx, UpdateLocaleCommand, ()>(f.command_ctx().clone(), cmd)
        .await?;

    // Assert
    // L'exécution réussit mais ne produit aucune mutation (No-Op transactionnel)
    f.account_assertions()
        .assert_account_state(f.account_id(), |acc| {
            assert_eq!(
                acc.version(),
                version_snapshot,
                "La version ne doit pas bouger si la locale était déjà identique"
            );
        })
        .await;

    // Aucun événement produit puisque l'état est resté identique
    f.account_assertions()
        .assert_no_events_for(f.account_id())
        .await;

    Ok(())
}
