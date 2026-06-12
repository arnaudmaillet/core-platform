// crates/account/src/application/use_cases/access_management/verify_phone/verify_phone_test.rs

use account::commands::access_management::VerifyPhoneCommand;
use account::context::AccountCommandCtx;
use account::events::AccountEvent;
use account::repositories::{GlobalIdentityRegistration, OtpRepository};
use account::types::{AccountState, RegistrationIdentifier};
use account_test_utils::asserts::AccountRepositoryAsserts;

use account_test_utils::AccountTestFixture;
use shared_kernel::command::CommandTarget;
use shared_kernel::core::{ErrorCode, Result, Versioned};
use shared_kernel::idempotency::IdempotencyRepository;
use shared_kernel::types::Phone;
use uuid::Uuid;

#[tokio::test]
async fn test_verify_phone_success() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();
    let code = "123456";
    let phone = Phone::try_new("+33612345678")?;

    // 1. On prépare un compte avec téléphone non vérifié au niveau régional
    let account = f
        .builder()?
        .with_state(AccountState::UNVERIFIED)
        .with_phone(phone.clone())
        .build()?;
    let version_snapshot = account.version();
    f.account_repo().insert(account.clone());

    // On peuple le registre global en état UNVERIFIED
    f.global_registry()
        .insert_fixture(GlobalIdentityRegistration {
            account_id: f.account_id(),
            region: f.region(),
            sub_id: account.identity().sub_id().cloned(),
            identifiers: RegistrationIdentifier::from_phone(phone),
            state: AccountState::UNVERIFIED,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        })
        .await;

    // Simulation du code OTP dans le cache Redis de test
    f.otp_repo().seed_code(f.account_id(), "phone", code);

    let cmd = VerifyPhoneCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.account_id(), version_snapshot),
        region: f.region(),
        code: code.to_string(),
    };

    // Act
    f.bus()
        .execute::<AccountCommandCtx, VerifyPhoneCommand, ()>(f.command_ctx().clone(), cmd)
        .await?;

    // Assert
    f.account_assertions()
        .assert_account_state(f.account_id(), |acc| {
            assert_eq!(*acc.identity().state(), AccountState::ACTIVE);
            assert!(acc.identity().phone_verified_at().is_some());
            assert_eq!(acc.version(), version_snapshot + 1);
        })
        .await;

    let cached_code = f.otp_repo().get_code(&f.account_id(), "phone").await?;
    assert!(
        cached_code.is_none(),
        "L'OTP téléphone aurait dû être supprimé après une vérification réussie"
    );

    f.assert_outbox(1, Some(AccountEvent::PHONE_VERIFIED));

    Ok(())
}

#[tokio::test]
async fn test_verify_phone_fails_if_otp_invalid() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();
    let phone = Phone::try_new("+33612345678")?;

    // Un code est stocké, mais l'utilisateur fournit un mauvais code
    let account = f
        .builder()?
        .with_state(AccountState::UNVERIFIED)
        .with_phone(phone)
        .build()?;
    let version_snapshot = account.version();
    f.account_repo().insert(account);
    f.otp_repo().seed_code(f.account_id(), "phone", "123456");

    let cmd = VerifyPhoneCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.account_id(), version_snapshot),
        region: f.region(),
        code: "654321".to_string(), // Mauvais code fourni
    };

    // Act
    let result = f
        .bus()
        .execute::<AccountCommandCtx, VerifyPhoneCommand, ()>(f.command_ctx().clone(), cmd)
        .await;

    // Assert
    match result {
        Err(e) => assert_eq!(e.code, ErrorCode::ValidationFailed),
        Ok(_) => panic!("Le cas d'usage aurait dû échouer : code OTP invalide"),
    }

    // L'agrégat n'a subi aucun changement d'état ou de version
    f.account_assertions()
        .assert_account_state(f.account_id(), |acc| {
            assert_eq!(*acc.identity().state(), AccountState::UNVERIFIED);
            assert_eq!(acc.version(), version_snapshot);
        })
        .await;

    // Le code doit rester disponible pour une nouvelle tentative
    let cached_code = f.otp_repo().get_code(&f.account_id(), "phone").await?;
    assert!(
        cached_code.is_some(),
        "Le code OTP ne doit pas être consommé s'il est invalide"
    );

    f.account_assertions()
        .assert_no_events_for(f.account_id())
        .await;

    Ok(())
}

#[tokio::test]
async fn test_verify_phone_fails_if_otp_expired() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();
    let phone = Phone::try_new("+33612345678")?;

    // Aucun code provisionné dans le stub (simule l'expiration Redis)
    let account = f
        .builder()?
        .with_state(AccountState::UNVERIFIED)
        .with_phone(phone)
        .build()?;
    let version_snapshot = account.version();
    f.account_repo().insert(account);

    let cmd = VerifyPhoneCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.account_id(), version_snapshot),
        region: f.region(),
        code: "123456".to_string(),
    };

    // Act
    let result = f
        .bus()
        .execute::<AccountCommandCtx, VerifyPhoneCommand, ()>(f.command_ctx().clone(), cmd)
        .await;

    // Assert
    match result {
        Err(e) => assert_eq!(e.code, ErrorCode::ValidationFailed),
        Ok(_) => panic!("Le cas d'usage aurait dû échouer : code OTP expiré ou non-existent"),
    }

    f.account_assertions()
        .assert_no_events_for(f.account_id())
        .await;

    Ok(())
}

#[tokio::test]
async fn test_verify_phone_technical_idempotency() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();
    let cmd_id = Uuid::new_v4();
    let phone = Phone::try_new("+33612345678")?;

    // La commande a déjà été marquée comme traitée au premier rideau
    f.idempotency_repo().save(None, &cmd_id).await?;

    let account = f
        .builder()?
        .with_state(AccountState::UNVERIFIED)
        .with_phone(phone)
        .build()?;
    let version_snapshot = account.version();
    f.account_repo().insert(account);
    f.otp_repo().seed_code(f.account_id(), "phone", "123456");

    let cmd = VerifyPhoneCommand {
        command_id: cmd_id,
        target: CommandTarget::versioned(f.account_id(), version_snapshot),
        region: f.region(),
        code: "123456".to_string(),
    };

    // Act
    let result = f
        .bus()
        .execute::<AccountCommandCtx, VerifyPhoneCommand, ()>(f.command_ctx().clone(), cmd)
        .await;

    // Assert
    assert!(
        result.is_ok(),
        "L'idempotence technique doit court-circuiter de façon transparente (Ok)"
    );

    // L'état reste identique
    f.account_assertions()
        .assert_account_state(f.account_id(), |acc| {
            assert_eq!(*acc.identity().state(), AccountState::UNVERIFIED);
            assert!(acc.identity().phone_verified_at().is_none());
            assert_eq!(acc.version(), version_snapshot);
        })
        .await;

    f.account_assertions()
        .assert_no_events_for(f.account_id())
        .await;

    Ok(())
}
