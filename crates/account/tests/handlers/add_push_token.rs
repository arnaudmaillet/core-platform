
use account::commands::settings::AddPushTokenCommand;
use account::context::AccountCommandContext;
use account::events::AccountEvent;
use account::types::AccountState;
use account_test_utils::AccountTestFixture;
use shared_kernel::{
    command::CommandTarget,
    core::{Error, Result, Versioned},
    messaging::EventEmitter,
    security::PushToken,
};
use shared_kernel_test_utils::repositories::TransactionManagerStub;
use uuid::Uuid;

#[tokio::test]
async fn test_add_push_token_success() -> Result<()> {
    let f = AccountTestFixture::new();
    let token = PushToken::try_new("valid_push_token_long_enough")?;

    // 1. Arrange : Compte actif sans tokens
    let account = f.builder()?.with_state(AccountState::ACTIVE).build()?;

    let version_snapshot = account.version();
    f.account_repo().insert(account);

    let cmd = AddPushTokenCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::new(f.account_id(), f.region(), version_snapshot),
        token: token.clone(),
    };

    // 2. Act
    f.bus()
        .execute::<AccountCommandContext<TransactionManagerStub>, AddPushTokenCommand, ()>(f.command_ctx().clone(), cmd)
        .await?;

    // 3. Assert
    f.assert_account(|acc| {
        assert!(acc.settings().push_tokens().contains(&token));
        assert_eq!(acc.version(), version_snapshot + 1);
    })
    .await?;

    f.assert_outbox(1, Some(AccountEvent::PUSH_TOKEN_ADDED));

    Ok(())
}

#[tokio::test]
async fn test_add_push_token_technical_idempotency() -> Result<()> {
    let f = AccountTestFixture::new();
    let cmd_id = Uuid::new_v4();
    let token = PushToken::try_new("idempotent_token_123")?;

    // Arrange : Commande déjà vue par l'infra
    f.idempotency_repo().seed(cmd_id);

    let account = f.builder()?.with_state(AccountState::ACTIVE).build()?;
    let version_snapshot = account.version();
    f.account_repo().insert(account);

    let cmd = AddPushTokenCommand {
        command_id: cmd_id,
        target: CommandTarget::new(f.account_id(), f.region(), version_snapshot),
        token: token.clone(),
    };

    // Act
    let result = f
        .bus()
        .execute::<AccountCommandContext<TransactionManagerStub>, AddPushTokenCommand, ()>(f.command_ctx().clone(), cmd)
        .await;

    // Assert
    assert!(
        result.is_ok(),
        "L'idempotence technique doit être transparente (Ok)"
    );
    f.assert_account(|acc| {
        assert!(!acc.settings().push_tokens().contains(&token));
        assert_eq!(acc.version(), version_snapshot);
    })
    .await?;
    f.assert_outbox(0, None);

    Ok(())
}

#[tokio::test]
async fn test_add_push_token_business_idempotency() -> Result<()> {
    let f = AccountTestFixture::new();
    let token = PushToken::try_new("existing_token_123")?;

    // 1. Arrange : Token déjà présent dans l'agrégat
    let account = f.builder()?.with_state(AccountState::ACTIVE).build()?;

    // On utilise une petite closure de test pour injecter le token sans passer par le bus
    let mut account = account;
    account.add_push_token(token.clone())?;
    account.pull_events(); // On vide l'event du setup

    let version_snapshot = account.version();
    f.account_repo().insert(account);

    let cmd = AddPushTokenCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::new(f.account_id(), f.region(), version_snapshot),
        token,
    };

    // 2. Act
    f.bus()
        .execute::<AccountCommandContext<TransactionManagerStub>, AddPushTokenCommand, ()>(f.command_ctx().clone(), cmd)
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

#[tokio::test]
async fn test_add_push_token_succeeds_after_retry() -> Result<()> {
    let f = AccountTestFixture::new();
    let account = f.builder()?.with_state(AccountState::ACTIVE).build()?;
    let version_snapshot = account.version();
    let token = PushToken::try_new("retry_token_123")?;
    f.account_repo().insert(account);

    // On simule UNE erreur de concurrence.
    // Le Stub la renverra une fois, puis redeviendra normal au retry.
    f.account_repo()
        .set_error_once(Error::concurrency_conflict("Database high pressure"));

    let cmd = AddPushTokenCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::new(f.account_id(), f.region(), version_snapshot),
        token: token.clone(),
    };

    // 2. Act : Le bus doit absorber le conflit et retenter l'opération
    let result = f
        .bus()
        .execute::<AccountCommandContext<TransactionManagerStub>, AddPushTokenCommand, ()>(f.command_ctx().clone(), cmd)
        .await;

    // 3. Assert : Succès attendu !
    assert!(
        result.is_ok(),
        "Le bus aurait dû réussir après le retry automatique"
    );

    f.assert_account(|acc| {
        // Le token doit être présent car l'opération a fini par réussir
        assert!(acc.settings().push_tokens().contains(&token));
        // La version doit être incrémentée (version initiale + 1)
        assert_eq!(acc.version(), version_snapshot + 1);
    })
    .await?;

    // Un événement doit être présent dans l'outbox
    f.assert_outbox(1, Some(AccountEvent::PUSH_TOKEN_ADDED));
    Ok(())
}
