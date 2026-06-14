// crates/account/src/application/use_cases/settings/remove_push_token/remove_push_token_use_case_test.rs

use account::commands::settings::RemovePushTokenCommand;
use account::context::AccountCommandCtx;
use account::entities::AccountSettingsBuilder;
use account::events::AccountEvent;
use account::types::AccountState;
use account_test_utils::asserts::AccountRepositoryAsserts;

use account_test_utils::AccountTestFixture;
use shared_kernel::{
    command::CommandTarget,
    core::{Result, Versioned},
    idempotency::IdempotencyRepository,
    security::PushToken,
};
use uuid::Uuid;

#[tokio::test]
async fn test_remove_push_token_success() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();
    let token_to_keep = PushToken::try_new("token_keep_456")?;
    let token_to_remove = PushToken::try_new("token_remove_123")?;

    // 1. On prépare un compte actif contenant déjà deux tokens distincts
    let account = f
        .builder()?
        .with_state(AccountState::ACTIVE)
        .settings(|s: AccountSettingsBuilder| {
            s.with_tokens(vec![token_to_remove.clone(), token_to_keep.clone()])
        })
        .build()?;

    let version_snapshot = account.version();
    f.account_repo().insert(account);

    let cmd = RemovePushTokenCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.account_id(), version_snapshot),
        region: f.server_region(),
        token: token_to_remove.clone(),
    };

    // Act
    f.bus()
        .execute::<AccountCommandCtx, RemovePushTokenCommand, ()>(f.command_ctx().clone(), cmd)
        .await?;

    // Assert
    f.account_assertions()
        .assert_account_state(f.account_id(), |acc| {
            let tokens = acc.settings().push_tokens();
            assert!(
                !tokens.contains(&token_to_remove),
                "Le token ciblé aurait dû être supprimé"
            );
            assert!(
                tokens.contains(&token_to_keep),
                "Le token non ciblé aurait dû être conservé"
            );
            assert_eq!(acc.version(), version_snapshot + 1);
        })
        .await;

    f.assert_outbox(1, Some(AccountEvent::PUSH_TOKEN_REMOVED));

    Ok(())
}

#[tokio::test]
async fn test_remove_push_token_technical_idempotency() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();
    let cmd_id = Uuid::new_v4();
    let token = PushToken::try_new("token_test_123")?;

    // On simule une commande déjà interceptée au premier rideau (Idempotency Barrière)
    f.idempotency_repo().save(None, &cmd_id).await?;

    let account = f.builder()?.with_state(AccountState::ACTIVE).build()?;
    let version_snapshot = account.version();
    f.account_repo().insert(account);

    let cmd = RemovePushTokenCommand {
        command_id: cmd_id,
        target: CommandTarget::versioned(f.account_id(), version_snapshot),
        region: f.server_region(),
        token: token.clone(),
    };

    // Act
    let result = f
        .bus()
        .execute::<AccountCommandCtx, RemovePushTokenCommand, ()>(f.command_ctx().clone(), cmd)
        .await;

    // Assert
    assert!(
        result.is_ok(),
        "L'idempotence technique doit court-circuiter de façon transparente (Ok)"
    );

    // L'agrégat en base ou stub n'a subi aucune écriture additionnelle ni mutation
    f.account_assertions()
        .assert_account_state(f.account_id(), |acc| {
            assert_eq!(
                acc.version(),
                version_snapshot,
                "La version ne doit pas changer"
            );
        })
        .await;

    // L'outbox locale reste intacte et vierge d'événements dupliqués
    f.account_assertions()
        .assert_no_events_for(f.account_id())
        .await;

    Ok(())
}

#[tokio::test]
async fn test_remove_push_token_business_idempotency() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();
    let non_existent_token = PushToken::try_new("valid_but_missing_token")?;

    // Idempotence métier : on demande la suppression d'un token qui n'est pas présent dans l'agrégat
    let account = f.builder()?.with_state(AccountState::ACTIVE).build()?;
    let version_snapshot = account.version();
    f.account_repo().insert(account);

    let cmd = RemovePushTokenCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.account_id(), version_snapshot),
        region: f.server_region(),
        token: non_existent_token.clone(),
    };

    // Act
    f.bus()
        .execute::<AccountCommandCtx, RemovePushTokenCommand, ()>(f.command_ctx().clone(), cmd)
        .await?;

    // Assert
    // L'exécution réussit mais ne produit aucune mutation (No-Op transactionnel)
    f.account_assertions()
        .assert_account_state(f.account_id(), |acc| {
            assert_eq!(
                acc.version(),
                version_snapshot,
                "La version ne doit pas changer si aucun token n'a été retiré"
            );
            assert!(acc.settings().push_tokens().is_empty());
        })
        .await;

    // Aucun événement produit puisque l'état est inchangé
    f.account_assertions()
        .assert_no_events_for(f.account_id())
        .await;

    Ok(())
}
