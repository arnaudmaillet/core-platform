// crates/account/src/application/use_cases/access_management/verify_email/verify_email_test.rs

use account::commands::access_management::VerifyEmailCommand;
use account::context::AccountCommandCtx;
use account::events::AccountEvent;
use account::repositories::{GlobalIdentityRegistration, OtpRepository};
use account::types::{AccountState, RegistrationIdentifier};
use account_test_utils::asserts::AccountRepositoryAsserts;

use account_test_utils::AccountTestFixture;
use shared_kernel::command::CommandTarget;
use shared_kernel::core::{ErrorCode, Result, Versioned};
use shared_kernel::idempotency::IdempotencyRepository;
use shared_kernel::types::Email;
use uuid::Uuid;

#[tokio::test]
async fn test_verify_email_success() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();
    let code = "123456";
    let email = Email::try_new("test@example.com")?;

    // 1. Compte non vérifié au niveau régional
    let account = f.builder()?.with_state(AccountState::UNVERIFIED).build()?;
    let version_snapshot = account.version();
    f.account_repo().insert(account.clone());

    // Alignement : On peuple le stub global
    f.global_registry()
        .insert_fixture(GlobalIdentityRegistration {
            account_id: f.account_id(),
            region: f.region(),
            sub_id: account.identity().sub_id().cloned(),
            identifiers: RegistrationIdentifier::from_email(email),
            state: AccountState::UNVERIFIED,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        })
        .await;

    // Simulation du code OTP dans le cache Redis de test
    f.otp_repo().seed_code(f.account_id(), "email", code);

    let cmd = VerifyEmailCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.account_id(), version_snapshot),
        region: f.region(),
        code: code.to_string(),
    };

    // Act
    f.bus()
        .execute::<AccountCommandCtx, VerifyEmailCommand, ()>(f.command_ctx().clone(), cmd)
        .await?;

    // Assert
    f.account_assertions()
        .assert_account_state(f.account_id(), |acc| {
            assert_eq!(*acc.identity().state(), AccountState::ACTIVE);
            assert_eq!(acc.version(), version_snapshot + 1);
        })
        .await;

    assert!(
        f.otp_repo()
            .get_code(&f.account_id(), "email")
            .await?
            .is_none(),
        "L'OTP aurait dû être invalidé après une vérification réussie"
    );

    f.assert_outbox(1, Some(AccountEvent::EMAIL_VERIFIED));

    Ok(())
}

#[tokio::test]
async fn test_verify_email_fails_if_otp_invalid() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();

    // Un code est stocké, mais l'utilisateur fournit un mauvais code
    let account = f.builder()?.with_state(AccountState::UNVERIFIED).build()?;
    let version_snapshot = account.version();
    f.account_repo().insert(account);
    f.otp_repo().seed_code(f.account_id(), "email", "123456");

    let cmd = VerifyEmailCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.account_id(), version_snapshot),
        region: f.region(),
        code: "654321".to_string(), // Mauvais code !
    };

    // Act
    let result = f
        .bus()
        .execute::<AccountCommandCtx, VerifyEmailCommand, ()>(f.command_ctx().clone(), cmd)
        .await;

    // Assert
    match result {
        Err(e) => assert_eq!(e.code, ErrorCode::ValidationFailed),
        Ok(_) => panic!("Le cas d'usage aurait dû échouer : code OTP invalide"),
    }

    // Le compte n'a pas bougé
    f.account_assertions()
        .assert_account_state(f.account_id(), |acc| {
            assert_eq!(*acc.identity().state(), AccountState::UNVERIFIED);
            assert_eq!(acc.version(), version_snapshot);
        })
        .await;

    // L'OTP invalide n'est pas supprimé pour permettre à l'utilisateur de retenter sa chance
    assert!(
        f.otp_repo()
            .get_code(&f.account_id(), "email")
            .await?
            .is_some(),
        "Le code OTP ne doit pas être consommé s'il est invalide"
    );

    f.account_assertions()
        .assert_no_events_for(f.account_id())
        .await;

    Ok(())
}

#[tokio::test]
async fn test_verify_email_fails_if_otp_expired() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();

    // Zéro code en cache Redis (simule l'expiration du TTL)
    let account = f.builder()?.with_state(AccountState::UNVERIFIED).build()?;
    let version_snapshot = account.version();
    f.account_repo().insert(account);

    let cmd = VerifyEmailCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.account_id(), version_snapshot),
        region: f.region(),
        code: "123456".to_string(),
    };

    // Act
    let result = f
        .bus()
        .execute::<AccountCommandCtx, VerifyEmailCommand, ()>(f.command_ctx().clone(), cmd)
        .await;

    // Assert
    match result {
        Err(e) => assert_eq!(e.code, ErrorCode::ValidationFailed),
        Ok(_) => panic!("Le cas d'usage aurait dû échouer : code OTP expiré/absent"),
    }

    f.account_assertions()
        .assert_no_events_for(f.account_id())
        .await;

    Ok(())
}

#[tokio::test]
async fn test_verify_email_technical_idempotency() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();
    let cmd_id = Uuid::new_v4();

    // La commande a déjà été marquée comme traitée au premier rideau
    f.idempotency_repo().save(None, &cmd_id).await?;

    let account = f.builder()?.with_state(AccountState::UNVERIFIED).build()?;
    let version_snapshot = account.version();
    f.account_repo().insert(account);
    f.otp_repo().seed_code(f.account_id(), "email", "123456");

    let cmd = VerifyEmailCommand {
        command_id: cmd_id,
        target: CommandTarget::versioned(f.account_id(), version_snapshot),
        region: f.region(),
        code: "123456".to_string(),
    };

    // Act
    let result = f
        .bus()
        .execute::<AccountCommandCtx, VerifyEmailCommand, ()>(f.command_ctx().clone(), cmd)
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
            assert_eq!(acc.version(), version_snapshot);
        })
        .await;

    f.account_assertions()
        .assert_no_events_for(f.account_id())
        .await;

    Ok(())
}
