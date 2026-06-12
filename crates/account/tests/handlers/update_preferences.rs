// crates/account/src/application/use_cases/settings/update_preferences/update_preferences_use_case_test.rs

use account::commands::settings::UpdatePreferencesCommand;
use account::context::AccountCommandCtx;
use account::entities::AccountSettingsBuilder;
use account::events::AccountEvent;
use account::types::{AccountState, AppearancePreferences, ThemeMode};
use account_test_utils::asserts::AccountRepositoryAsserts;

use account_test_utils::AccountTestFixture;
use shared_kernel::command::CommandTarget;
use shared_kernel::core::{Result, Versioned};
use shared_kernel::idempotency::IdempotencyRepository;
use uuid::Uuid;

#[tokio::test]
async fn test_update_preferences_success() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();

    // 1. On prépare un compte actif
    let account = f.builder()?.with_state(AccountState::ACTIVE).build()?;
    let version_snapshot = account.version();
    f.account_repo().insert(account);

    let new_appearance = AppearancePreferences::builder()
        .with_theme(ThemeMode::Dark)
        .with_high_contrast(true)
        .build();

    let cmd = UpdatePreferencesCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.account_id(), version_snapshot),
        region: f.region(),
        privacy: None,
        notifications: None,
        appearance: Some(new_appearance.clone()),
    };

    // Act
    f.bus()
        .execute::<AccountCommandCtx, UpdatePreferencesCommand, ()>(f.command_ctx().clone(), cmd)
        .await?;

    // Assert
    f.account_assertions()
        .assert_account_state(f.account_id(), |acc| {
            assert_eq!(
                acc.settings().preferences().appearance().theme(),
                ThemeMode::Dark
            );
            assert!(acc.settings().preferences().appearance().high_contrast());
            assert_eq!(acc.version(), version_snapshot + 1);
        })
        .await;

    f.assert_outbox(1, Some(AccountEvent::APPEARANCE_PREFS_UPDATED));

    Ok(())
}

#[tokio::test]
async fn test_update_preferences_technical_idempotency() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();
    let cmd_id = Uuid::new_v4();

    // On simule une commande déjà traitée techniquement interceptée au premier rideau
    f.idempotency_repo().save(None, &cmd_id).await?;

    let account = f.builder()?.with_state(AccountState::ACTIVE).build()?;
    let version_snapshot = account.version();
    f.account_repo().insert(account);

    let cmd = UpdatePreferencesCommand {
        command_id: cmd_id,
        target: CommandTarget::versioned(f.account_id(), version_snapshot),
        region: f.region(),
        privacy: None,
        notifications: None,
        appearance: None,
    };

    // Act
    let result = f
        .bus()
        .execute::<AccountCommandCtx, UpdatePreferencesCommand, ()>(f.command_ctx().clone(), cmd)
        .await;

    // Assert
    assert!(
        result.is_ok(),
        "L'idempotence technique doit court-circuiter de façon transparente (Ok)"
    );

    // L'agrégat en base n'a subi aucune mutation
    f.account_assertions()
        .assert_account_state(f.account_id(), |acc| {
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
async fn test_update_preferences_business_idempotency() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();

    let initial_appearance = AppearancePreferences::builder()
        .with_theme(ThemeMode::System)
        .with_high_contrast(true)
        .build();

    // Idempotence métier : Le compte possède déjà exactement ces préférences
    let account = f
        .builder()?
        .with_state(AccountState::ACTIVE)
        .settings(|s: AccountSettingsBuilder| s.with_appearance(initial_appearance.clone()))
        .build()?;

    let version_snapshot = account.version();
    f.account_repo().insert(account);

    let cmd = UpdatePreferencesCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.account_id(), version_snapshot),
        region: f.region(),
        privacy: None,
        notifications: None,
        appearance: Some(initial_appearance),
    };

    // Act
    f.bus()
        .execute::<AccountCommandCtx, UpdatePreferencesCommand, ()>(f.command_ctx().clone(), cmd)
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
