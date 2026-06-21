// crates/account/tests/handlers/change_email.rs

use account_old::commands::settings::ChangeEmailCommand;
use account_old::context::AccountCommandCtx;
use account_old::events::AccountEvent;
use account_old::types::{AccountState, RegistrationIdentifier};
use account_test_utils::asserts::AccountRepositoryAsserts;

use account_test_utils::AccountTestFixture;
use shared_kernel::command::CommandTarget;
use shared_kernel::core::{Error, ErrorCode, Result, Versioned};
use shared_kernel::idempotency::IdempotencyRepository;
use shared_kernel::types::Email;
use uuid::Uuid;

#[tokio::test]
async fn test_change_email_success() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();
    let old_email = Email::try_new("old@test.com")?;
    let new_email = Email::try_new("new@test.com")?;

    let account = f
        .builder()?
        .with_state(AccountState::ACTIVE)
        .with_email(old_email.clone())
        .build()?;

    let version_snapshot = account.version();
    f.account_repo().insert(account);

    f.global_registry()
        .insert_fixture(account_old::repositories::GlobalIdentityRegistration {
            account_id: f.account_id(),
            region: f.server_region(),
            sub_id: None,
            identifiers: RegistrationIdentifier::from_email(old_email),
            state: AccountState::ACTIVE,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        })
        .await;

    let cmd = ChangeEmailCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.account_id(), version_snapshot),
        region: f.server_region(),
        new_email: new_email.clone(),
    };

    // Act
    f.bus()
        .execute::<AccountCommandCtx, ChangeEmailCommand, ()>(f.command_ctx().clone(), cmd)
        .await?;

    // Assert
    f.account_assertions()
        .assert_account_state(f.account_id(), |acc| {
            assert_eq!(acc.identity().email(), Some(&new_email));
            assert_eq!(acc.version(), version_snapshot + 1);
        })
        .await;

    f.assert_outbox(1, Some(AccountEvent::EMAIL_CHANGED));

    Ok(())
}

#[tokio::test]
async fn test_change_email_technical_idempotency() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();
    let cmd_id = Uuid::new_v4();
    let requested_email = Email::try_new("other@test.com")?;

    // On simule une commande déjà traitée et validée par le premier rideau d'idempotence
    f.idempotency_repo().save(None, &cmd_id).await?;

    let account = f.builder()?.with_state(AccountState::ACTIVE).build()?;
    let version_snapshot = account.version();
    f.account_repo().insert(account);

    let cmd = ChangeEmailCommand {
        command_id: cmd_id,
        target: CommandTarget::versioned(f.account_id(), version_snapshot),
        region: f.server_region(),
        new_email: requested_email.clone(),
    };

    // Act
    let result = f
        .bus()
        .execute::<AccountCommandCtx, ChangeEmailCommand, ()>(f.command_ctx().clone(), cmd)
        .await;

    // Assert
    assert!(
        result.is_ok(),
        "L'idempotence technique doit court-circuiter de façon transparente (Ok)"
    );

    // L'agrégat n'a subi aucune mutation interne
    f.account_assertions()
        .assert_account_state(f.account_id(), |acc| {
            assert_ne!(acc.identity().email(), Some(&requested_email));
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
async fn test_change_email_business_idempotency() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();
    let email = Email::try_new("same@test.com")?;

    // Idempotence métier : le compte possède déjà l'email demandé
    let account = f
        .builder()?
        .with_state(AccountState::ACTIVE)
        .with_email(email.clone())
        .build()?;

    let version_snapshot = account.version();
    f.account_repo().insert(account);

    f.global_registry()
        .insert_fixture(account_old::repositories::GlobalIdentityRegistration {
            account_id: f.account_id(),
            region: f.server_region(),
            sub_id: None,
            identifiers: RegistrationIdentifier::from_email(email.clone()),
            state: AccountState::ACTIVE,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        })
        .await;

    let cmd = ChangeEmailCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.account_id(), version_snapshot),
        region: f.server_region(),
        new_email: email,
    };

    // Act
    f.bus()
        .execute::<AccountCommandCtx, ChangeEmailCommand, ()>(f.command_ctx().clone(), cmd)
        .await?;

    // Assert
    // Pas de modification d'état ni d'incrément de version (No-Op transactionnel)
    f.account_assertions()
        .assert_account_state(f.account_id(), |acc| {
            assert_eq!(acc.version(), version_snapshot);
        })
        .await;

    // Aucun événement métier produit puisque l'état est inchangé
    f.account_assertions()
        .assert_no_events_for(f.account_id())
        .await;

    Ok(())
}

#[tokio::test]
async fn test_change_email_forbidden_when_restricted() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();
    let requested_email = Email::try_new("new@test.com")?;
    let old_email = Email::try_new("old@test.com")?;

    // Un compte banni ne peut pas modifier ses réglages
    let account = f
        .builder()?
        .with_state(AccountState::BANNED)
        .with_email(old_email.clone())
        .build()?;

    let version_snapshot = account.version();
    f.account_repo().insert(account);

    f.global_registry()
        .insert_fixture(account_old::repositories::GlobalIdentityRegistration {
            account_id: f.account_id(),
            region: f.server_region(),
            sub_id: None,
            identifiers: RegistrationIdentifier::from_email(old_email),
            state: AccountState::BANNED,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        })
        .await;

    let cmd = ChangeEmailCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.account_id(), version_snapshot),
        region: f.server_region(),
        new_email: requested_email.clone(),
    };

    // Act
    let result = f
        .bus()
        .execute::<AccountCommandCtx, ChangeEmailCommand, ()>(f.command_ctx().clone(), cmd)
        .await;

    // Assert
    match result {
        Err(e) => {
            assert_eq!(e.code, ErrorCode::Forbidden);
        }
        Ok(_) => panic!(
            "Le cas d'usage aurait dû échouer : un compte banni ne peut pas modifier son email"
        ),
    }

    // Sécurité de l'état : l'invariant a tenu bon, aucune écriture
    f.account_assertions()
        .assert_account_state(f.account_id(), |acc| {
            assert_ne!(acc.identity().email(), Some(&requested_email));
            assert_eq!(acc.version(), version_snapshot);
        })
        .await;

    f.account_assertions()
        .assert_no_events_for(f.account_id())
        .await;

    Ok(())
}

#[tokio::test]
async fn test_change_email_succeeds_after_retry() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();
    let requested_email = Email::try_new("b@c.com")?;
    let old_email = Email::try_new("old@test.com")?;

    let account = f
        .builder()?
        .with_state(AccountState::ACTIVE)
        .with_email(old_email.clone())
        .build()?;

    let version_snapshot = account.version();
    f.account_repo().insert(account);

    f.global_registry()
        .insert_fixture(account_old::repositories::GlobalIdentityRegistration {
            account_id: f.account_id(),
            region: f.server_region(),
            sub_id: None,
            identifiers: RegistrationIdentifier::from_email(old_email),
            state: AccountState::ACTIVE,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        })
        .await;

    // Simulation d'une erreur de concurrence transitoire (OCC conflict)
    f.account_repo().set_error_once(Error::concurrency_conflict(
        "Version mismatch / OCC Concurrency conflict",
    ));

    let cmd = ChangeEmailCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.account_id(), version_snapshot),
        region: f.server_region(),
        new_email: requested_email.clone(),
    };

    // Act : Le middleware with_retry du bus doit absorber l'erreur transitoire et retenter
    let result = f
        .bus()
        .execute::<AccountCommandCtx, ChangeEmailCommand, ()>(f.command_ctx().clone(), cmd)
        .await;

    // Assert
    assert!(
        result.is_ok(),
        "Le bus de commande aurait dû réussir après le retry automatique"
    );

    f.account_assertions()
        .assert_account_state(f.account_id(), |acc| {
            assert_eq!(acc.identity().email(), Some(&requested_email));
            assert_eq!(acc.version(), version_snapshot + 1);
        })
        .await;

    // Un seul événement propre est poussé dans l'outbox après la tentative résiliente réussie
    f.assert_outbox(1, Some(AccountEvent::EMAIL_CHANGED));

    Ok(())
}
