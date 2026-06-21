// crates/account/tests/handlers/change_phone.rs

use account::commands::settings::ChangePhoneCommand;
use account::context::AccountCommandCtx;
use account::events::AccountEvent;
use account::repositories::GlobalIdentityRegistration;
use account::types::{AccountState, RegistrationIdentifier};
use account_test_utils::asserts::AccountRepositoryAsserts;

use account_test_utils::AccountTestFixture;
use shared_kernel::command::CommandTarget;
use shared_kernel::core::{Error, ErrorCode, Result, Versioned};
use shared_kernel::idempotency::IdempotencyRepository;
use shared_kernel::types::Phone;
use uuid::Uuid;

#[tokio::test]
async fn test_change_phone_success() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();
    let old_phone = Phone::try_new("+33612345678")?;
    let new_phone = Phone::try_new("+33687654321")?;

    let account = f
        .builder()?
        .with_state(AccountState::ACTIVE)
        .with_phone(old_phone.clone())
        .build()?;

    let version_snapshot = account.version();
    f.account_repo().insert(account);

    f.global_registry()
        .insert_fixture(GlobalIdentityRegistration {
            account_id: f.account_id(),
            region: f.server_region(),
            sub_id: None,
            identifiers: RegistrationIdentifier::from_phone(old_phone),
            state: AccountState::ACTIVE,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        })
        .await;

    let cmd = ChangePhoneCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.account_id(), version_snapshot),
        region: f.server_region(),
        new_phone: new_phone.clone(),
    };

    // Act
    f.bus()
        .execute::<AccountCommandCtx, ChangePhoneCommand, ()>(f.command_ctx().clone(), cmd)
        .await?;

    // Assert
    f.account_assertions()
        .assert_account_state(f.account_id(), |acc| {
            assert_eq!(acc.identity().phone(), Some(&new_phone));
            assert_eq!(acc.version(), version_snapshot + 1);
        })
        .await;

    // L'infrastructure transactionnelle unifiée a flushé l'événement vers l'Outbox globale
    f.assert_outbox(1, Some(AccountEvent::PHONE_CHANGED));

    Ok(())
}

#[tokio::test]
async fn test_change_phone_technical_idempotency() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();
    let cmd_id = Uuid::new_v4();
    let requested_phone = Phone::try_new("+33600000000")?;

    // On simule une commande déjà traitée et validée par le premier rideau d'idempotence
    f.idempotency_repo().save(None, &cmd_id).await?;

    let account = f.builder()?.with_state(AccountState::ACTIVE).build()?;
    let version_snapshot = account.version();
    f.account_repo().insert(account);

    let cmd = ChangePhoneCommand {
        command_id: cmd_id,
        target: CommandTarget::versioned(f.account_id(), version_snapshot),
        region: f.server_region(),
        new_phone: requested_phone.clone(),
    };

    // Act
    let result = f
        .bus()
        .execute::<AccountCommandCtx, ChangePhoneCommand, ()>(f.command_ctx().clone(), cmd)
        .await;

    // Assert
    assert!(
        result.is_ok(),
        "L'idempotence technique doit court-circuiter de façon transparente (Ok)"
    );

    // L'agrégat n'a subi aucune mutation interne
    f.account_assertions()
        .assert_account_state(f.account_id(), |acc| {
            assert_ne!(acc.identity().phone(), Some(&requested_phone));
            assert_eq!(acc.version(), version_snapshot);
        })
        .await;

    // Aucun événement n'est ré-émis ou dupliqué
    f.account_assertions()
        .assert_no_events_for(f.account_id())
        .await;

    Ok(())
}

#[tokio::test]
async fn test_change_phone_business_idempotency() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();
    let phone = Phone::try_new("+33611111111")?;

    // Idempotence métier : le compte possède déjà le numéro de téléphone demandé
    let account = f
        .builder()?
        .with_state(AccountState::ACTIVE)
        .with_phone(phone.clone())
        .build()?;

    let version_snapshot = account.version();
    f.account_repo().insert(account);

    f.global_registry()
        .insert_fixture(account::repositories::GlobalIdentityRegistration {
            account_id: f.account_id(),
            region: f.server_region(),
            sub_id: None,
            identifiers: RegistrationIdentifier::from_phone(phone.clone()),
            state: AccountState::ACTIVE,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        })
        .await;

    let cmd = ChangePhoneCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.account_id(), version_snapshot),
        region: f.server_region(),
        new_phone: phone,
    };

    // Act
    f.bus()
        .execute::<AccountCommandCtx, ChangePhoneCommand, ()>(f.command_ctx().clone(), cmd)
        .await?;

    // Assert
    // Pas de modification d'état ni d'incrément de version (No-Op transactionnel)
    f.account_assertions()
        .assert_account_state(f.account_id(), |acc| {
            assert_eq!(acc.version(), version_snapshot);
        })
        .await;

    f.account_assertions()
        .assert_no_events_for(f.account_id())
        .await;

    Ok(())
}

#[tokio::test]
async fn test_change_phone_forbidden_when_restricted() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();
    let requested_phone = Phone::try_new("+33622222222")?;
    let old_phone = Phone::try_new("+33633333333")?;

    // Un compte banni ne peut pas modifier ses réglages d'identité
    let account = f
        .builder()?
        .with_state(AccountState::BANNED)
        .with_phone(old_phone.clone())
        .build()?;

    let version_snapshot = account.version();
    f.account_repo().insert(account);

    f.global_registry()
        .insert_fixture(account::repositories::GlobalIdentityRegistration {
            account_id: f.account_id(),
            region: f.server_region(),
            sub_id: None,
            identifiers: RegistrationIdentifier::from_phone(old_phone),
            state: AccountState::BANNED,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        })
        .await;

    let cmd = ChangePhoneCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.account_id(), version_snapshot),
        region: f.server_region(),
        new_phone: requested_phone.clone(),
    };

    // Act
    let result = f
        .bus()
        .execute::<AccountCommandCtx, ChangePhoneCommand, ()>(f.command_ctx().clone(), cmd)
        .await;

    // Assert
    match result {
        Err(e) => {
            assert_eq!(e.code, ErrorCode::Forbidden);
        }
        Ok(_) => panic!(
            "Le cas d'usage aurait dû échouer : un compte banni ne peut pas modifier son téléphone"
        ),
    }

    f.account_assertions()
        .assert_account_state(f.account_id(), |acc| {
            assert_ne!(acc.identity().phone(), Some(&requested_phone));
            assert_eq!(acc.version(), version_snapshot);
        })
        .await;

    f.account_assertions()
        .assert_no_events_for(f.account_id())
        .await;

    Ok(())
}

#[tokio::test]
async fn test_change_phone_succeeds_after_retry() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();
    let requested_phone = Phone::try_new("+33644444444")?;
    let old_phone = Phone::try_new("+33655555555")?;

    let account = f
        .builder()?
        .with_state(AccountState::ACTIVE)
        .with_phone(old_phone.clone())
        .build()?;

    let version_snapshot = account.version();
    f.account_repo().insert(account);

    f.global_registry()
        .insert_fixture(GlobalIdentityRegistration {
            account_id: f.account_id(),
            region: f.server_region(),
            sub_id: None,
            identifiers: RegistrationIdentifier::from_phone(old_phone),
            state: AccountState::ACTIVE,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        })
        .await;

    // Simulation d'une erreur de concurrence transitoire (OCC conflict) au premier jet
    f.account_repo().set_error_once(Error::concurrency_conflict(
        "Version mismatch / OCC Concurrency conflict",
    ));

    let cmd = ChangePhoneCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.account_id(), version_snapshot),
        region: f.server_region(),
        new_phone: requested_phone.clone(),
    };

    // Act : Le middleware retry absorbe et relance sur le sharding strict
    let result = f
        .bus()
        .execute::<AccountCommandCtx, ChangePhoneCommand, ()>(f.command_ctx().clone(), cmd)
        .await;

    // Assert
    assert!(
        result.is_ok(),
        "Le bus de commande aurait dû réussir après le retry automatique"
    );

    f.account_assertions()
        .assert_account_state(f.account_id(), |acc| {
            assert_eq!(acc.identity().phone(), Some(&requested_phone));
            assert_eq!(acc.version(), version_snapshot + 1);
        })
        .await;

    // Comme l'événement transite par l'Outbox globale à cause du pull applicatif en amont,
    // on assure la robustesse du typage sur l'outbox globale
    let events = f.outbox_events();
    assert!(events.contains(&AccountEvent::PHONE_CHANGED.to_string()));

    Ok(())
}
