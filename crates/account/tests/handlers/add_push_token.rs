// crates/account/src/application/use_cases/settings/add_push_token/add_push_token_use_case_test.rs

use account::commands::settings::AddPushTokenCommand;
use account::context::AccountCommandCtx;
use account::events::AccountEvent;
use account::types::AccountState;
use account_test_utils::asserts::AccountRepositoryAsserts;

use account_test_utils::AccountTestFixture;
use shared_kernel::{
    command::CommandTarget,
    core::{Error, Result, Versioned},
    idempotency::IdempotencyRepository,
    messaging::EventEmitter,
    security::PushToken,
};
use uuid::Uuid;

#[tokio::test]
async fn test_add_push_token_success() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();
    let token = PushToken::try_new("valid_push_token_long_enough")?;

    // 1. Compte actif initialisé sans tokens
    let account = f.builder()?.with_state(AccountState::ACTIVE).build()?;
    let version_snapshot = account.version();
    f.account_repo().insert(account);

    let cmd = AddPushTokenCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.account_id(), version_snapshot),
        region: f.server_region(),
        token: token.clone(),
    };

    // Act
    f.bus()
        .execute::<AccountCommandCtx, AddPushTokenCommand, ()>(f.command_ctx().clone(), cmd)
        .await?;

    // Assert
    f.account_assertions()
        .assert_account_state(f.account_id(), |acc| {
            assert!(acc.settings().push_tokens().contains(&token));
            assert_eq!(acc.version(), version_snapshot + 1);
        })
        .await;

    f.assert_outbox(1, Some(AccountEvent::PUSH_TOKEN_ADDED));

    Ok(())
}

#[tokio::test]
async fn test_add_push_token_technical_idempotency() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();
    let cmd_id = Uuid::new_v4();
    let token = PushToken::try_new("idempotent_token_123")?;

    // On simule une commande déjà interceptée au premier rideau (Idempotency Barrière)
    f.idempotency_repo().save(None, &cmd_id).await?;

    let account = f.builder()?.with_state(AccountState::ACTIVE).build()?;
    let version_snapshot = account.version();
    f.account_repo().insert(account);

    let cmd = AddPushTokenCommand {
        command_id: cmd_id,
        target: CommandTarget::versioned(f.account_id(), version_snapshot),
        region: f.server_region(),
        token: token.clone(),
    };

    // Act
    let result = f
        .bus()
        .execute::<AccountCommandCtx, AddPushTokenCommand, ()>(f.command_ctx().clone(), cmd)
        .await;

    // Assert
    assert!(
        result.is_ok(),
        "L'idempotence technique doit court-circuiter de façon transparente (Ok)"
    );

    // L'agrégat n'a subi aucune mutation interne, pas de token ajouté
    f.account_assertions()
        .assert_account_state(f.account_id(), |acc| {
            assert!(!acc.settings().push_tokens().contains(&token));
            assert_eq!(acc.version(), version_snapshot);
        })
        .await;

    // L'outbox locale reste vierge d'événements dupliqués
    f.account_assertions()
        .assert_no_events_for(f.account_id())
        .await;

    Ok(())
}

#[tokio::test]
async fn test_add_push_token_business_idempotency() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();
    let token = PushToken::try_new("existing_token_123")?;

    // Le token est déjà présent dans l'agrégat avant l'envoi de la commande
    let mut account = f.builder()?.with_state(AccountState::ACTIVE).build()?;
    account.add_push_token(token.clone())?;
    account.pull_events();

    let version_snapshot = account.version();
    f.account_repo().insert(account);

    let cmd = AddPushTokenCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.account_id(), version_snapshot),
        region: f.server_region(),
        token,
    };

    // Act
    f.bus()
        .execute::<AccountCommandCtx, AddPushTokenCommand, ()>(f.command_ctx().clone(), cmd)
        .await?;

    // Assert
    // Pas de modification d'état, pas d'incrément de version (No-Op transactionnel)
    f.account_assertions()
        .assert_account_state(f.account_id(), |acc| {
            assert_eq!(
                acc.version(),
                version_snapshot,
                "La version de l'agrégat ne doit pas changer"
            );
        })
        .await;

    // L'invariant d'unicité métier bloque l'émission de nouveaux événements
    f.account_assertions()
        .assert_no_events_for(f.account_id())
        .await;

    Ok(())
}

#[tokio::test]
async fn test_add_push_token_succeeds_after_retry() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();
    let account = f.builder()?.with_state(AccountState::ACTIVE).build()?;
    let version_snapshot = account.version();
    let token = PushToken::try_new("retry_token_123")?;
    f.account_repo().insert(account);

    // Simulation d'une erreur de concurrence transitoire (ex: Optimistic Locking Failure / OCC)
    // Le Stub lève l'erreur une fois lors du premier .exists / .save, puis fonctionne au retry.
    f.account_repo().set_error_once(Error::concurrency_conflict(
        "Database high pressure / OCC Concurrency conflict",
    ));

    let cmd = AddPushTokenCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.account_id(), version_snapshot),
        region: f.server_region(),
        token: token.clone(),
    };

    // Act : Le middleware with_retry du CommandBus intercepte ConcurrencyConflict et relance le flux
    let result = f
        .bus()
        .execute::<AccountCommandCtx, AddPushTokenCommand, ()>(f.command_ctx().clone(), cmd)
        .await;

    // Assert
    assert!(
        result.is_ok(),
        "Le middleware de retry du bus de commande aurait dû absorber l'erreur et réussir au second essai"
    );

    // L'état final après retry automatique est correct
    f.account_assertions()
        .assert_account_state(f.account_id(), |acc| {
            assert!(acc.settings().push_tokens().contains(&token));
            assert_eq!(acc.version(), version_snapshot + 1);
        })
        .await;

    // Un seul événement propre est poussé dans l'outbox après la tentative réussie
    f.assert_outbox(1, Some(AccountEvent::PUSH_TOKEN_ADDED));

    Ok(())
}
