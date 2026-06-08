use account::context::AccountCommandContext;
use account::events::AccountEvent;
use account::types::AccountState;
use account::{commands::settings::UpdateTimezoneCommand, entities::AccountSettingsBuilder};
use account_test_utils::AccountTestFixture;
use shared_kernel::{
    command::CommandTarget,
    core::{Result, Versioned},
    geo::Timezone,
};
use shared_kernel_test_utils::repositories::TransactionManagerStub;
use uuid::Uuid;

#[tokio::test]
async fn test_update_timezone_success() -> Result<()> {
    let f = AccountTestFixture::new();
    let initial_tz = Timezone::from_raw("UTC");
    let new_tz = Timezone::from_raw("Europe/Paris");

    // 1. Arrange : Compte actif en UTC
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
        region: f.region(),
        new_timezone: new_tz.clone(),
    };

    // 2. Act
    f.bus()
        .execute::<AccountCommandContext<TransactionManagerStub>, UpdateTimezoneCommand, ()>(
            f.command_ctx().clone(),
            cmd,
        )
        .await?;

    // 3. Assert
    f.assert_account(|acc| {
        assert_eq!(acc.settings().timezone(), &new_tz);
        assert_eq!(acc.version(), version_snapshot + 1);
    })
    .await?;

    f.assert_outbox(1, Some(AccountEvent::TIMEZONE_UPDATED));

    Ok(())
}

#[tokio::test]
async fn test_update_timezone_technical_idempotency() -> Result<()> {
    let f = AccountTestFixture::new();
    let cmd_id = Uuid::new_v4();
    let requested_tz = Timezone::from_raw("Europe/Paris");

    // Arrange : Commande déjà traitée par l'infrastructure
    f.idempotency_repo().seed(cmd_id);

    let account = f.builder()?.with_state(AccountState::ACTIVE).build()?;
    let version_snapshot = account.version();
    f.account_repo().insert(account);

    let cmd = UpdateTimezoneCommand {
        command_id: cmd_id,
        target: CommandTarget::versioned(f.account_id(), version_snapshot),
        region: f.region(),
        new_timezone: requested_tz.clone(),
    };

    // Act
    let result = f
        .bus()
        .execute::<AccountCommandContext<TransactionManagerStub>, UpdateTimezoneCommand, ()>(
            f.command_ctx().clone(),
            cmd,
        )
        .await;

    // Assert
    assert!(
        result.is_ok(),
        "L'idempotence technique doit être transparente (Ok)"
    );

    // Vérification d'intégrité
    f.assert_account(|acc| {
        assert_ne!(acc.settings().timezone(), &requested_tz);
        assert_eq!(acc.version(), version_snapshot);
    })
    .await?;

    f.assert_outbox(0, None);
    Ok(())
}

#[tokio::test]
async fn test_update_timezone_business_idempotency() -> Result<()> {
    let f = AccountTestFixture::new();
    let current_tz = Timezone::from_raw("Europe/Paris");

    // 1. Arrange : Compte possédant déjà cette timezone
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
        region: f.region(),
        new_timezone: current_tz,
    };

    // 2. Act
    f.bus()
        .execute::<AccountCommandContext<TransactionManagerStub>, UpdateTimezoneCommand, ()>(
            f.command_ctx().clone(),
            cmd,
        )
        .await?;

    // 3. Assert
    f.assert_account(|acc| {
        assert_eq!(
            acc.version(),
            version_snapshot,
            "La version ne doit pas bouger"
        );
    })
    .await?;

    f.assert_outbox(0, None);

    Ok(())
}
