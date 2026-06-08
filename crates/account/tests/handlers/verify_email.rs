// crates/account/src/application/use_cases/access_management/verify_email/verify_email_test.rs

use account::commands::access_management::VerifyEmailCommand;
use account::context::AccountCommandContext;
use account::events::AccountEvent;
use account::repositories::{GlobalIdentityRegistration, OtpRepository};
use account::types::{AccountState, RegistrationIdentifier};
use account_test_utils::AccountTestFixture;
use shared_kernel::command::CommandTarget;
use shared_kernel::core::{ErrorCode, Result, Versioned};
use shared_kernel::types::Email;
use shared_kernel_test_utils::repositories::TransactionManagerStub;
use uuid::Uuid;

#[tokio::test]
async fn test_verify_email_success() -> Result<()> {
    let f = AccountTestFixture::new();
    let code = "123456";
    let email = Email::try_new("test@example.com")?;

    // 1. Arrange : Un compte non vérifié au niveau régional
    let account = f.builder()?.with_state(AccountState::UNVERIFIED).build()?;
    let version_snapshot = account.version();
    f.account_repo().insert(account.clone());

    // 💡 ALIGNEMENT : On peuple le stub global avec la méthode que tu as codée
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
        .await; // 💡 Note : insert_fixture est asynchrone, ne pas oublier le .await !

    // On simule la présence du code OTP généré au préalable dans Redis
    f.otp_repo().seed_code(f.account_id(), "email", code);

    let cmd = VerifyEmailCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.account_id(), version_snapshot),
        region: f.region(),
        code: code.to_string(),
    };

    // 2. Act
    f.bus()
        .execute::<AccountCommandContext<TransactionManagerStub>, VerifyEmailCommand, ()>(
            f.command_ctx().clone(),
            cmd,
        )
        .await?;

    // 4. Assert : L'OTP à usage unique doit avoir été consommé/supprimé du cache
    assert!(
        f.otp_repo()
            .get_code(&f.account_id(), "email")
            .await?
            .is_none()
    );

    // 5. Assert : Projections événementielles (Outbox Kafka)
    f.assert_outbox(1, Some(AccountEvent::EMAIL_VERIFIED));

    Ok(())
}

#[tokio::test]
async fn test_verify_email_fails_if_otp_invalid() -> Result<()> {
    let f = AccountTestFixture::new();

    // Arrange : Un code est stocké, mais l'utilisateur en fournit un mauvais
    let account = f.builder()?.with_state(AccountState::UNVERIFIED).build()?;
    f.account_repo().insert(account);
    f.otp_repo().seed_code(f.account_id(), "email", "123456");

    let cmd = VerifyEmailCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.account_id(), 0),
        region: f.region(),
        code: "654321".to_string(), // Mauvais code !
    };

    // 2. Act
    let result = f
        .bus()
        .execute::<AccountCommandContext<TransactionManagerStub>, VerifyEmailCommand, ()>(
            f.command_ctx().clone(),
            cmd,
        )
        .await;

    // 3. Assert
    match result {
        Err(e) => assert_eq!(e.code, ErrorCode::ValidationFailed),
        Ok(_) => panic!("Should have failed: user provided an invalid OTP code"),
    }

    // Le compte ne doit pas avoir bougé
    f.assert_account(|acc| {
        assert_eq!(*acc.identity().state(), AccountState::UNVERIFIED);
        assert_eq!(acc.version(), 0);
    })
    .await?;

    // L'OTP invalide ne doit pas être supprimé (permettre à l'user de retenter sa chance)
    assert!(
        f.otp_repo()
            .get_code(&f.account_id(), "email")
            .await?
            .is_some()
    );
    f.assert_outbox(0, None);

    Ok(())
}

#[tokio::test]
async fn test_verify_email_fails_if_otp_expired() -> Result<()> {
    let f = AccountTestFixture::new();

    // Arrange : Zéro code en cache Redis (simule l'expiration du TTL)
    let account = f.builder()?.with_state(AccountState::UNVERIFIED).build()?;
    f.account_repo().insert(account);

    let cmd = VerifyEmailCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.account_id(), 0),
        region: f.region(),
        code: "123456".to_string(),
    };

    // 2. Act
    let result = f
        .bus()
        .execute::<AccountCommandContext<TransactionManagerStub>, VerifyEmailCommand, ()>(
            f.command_ctx().clone(),
            cmd,
        )
        .await;

    // 3. Assert
    match result {
        Err(e) => assert_eq!(e.code, ErrorCode::ValidationFailed),
        Ok(_) => panic!("Should have failed: OTP code is missing/expired in cache"),
    }

    f.assert_outbox(0, None);
    Ok(())
}

#[tokio::test]
async fn test_verify_email_technical_idempotency() -> Result<()> {
    let f = AccountTestFixture::new();
    let cmd_id = Uuid::new_v4();

    // Arrange : La commande a déjà été marquée comme traitée dans la table d'idempotence
    f.idempotency_repo().seed(cmd_id);

    let account = f.builder()?.with_state(AccountState::UNVERIFIED).build()?;
    f.account_repo().insert(account);
    f.otp_repo().seed_code(f.account_id(), "email", "123456");

    let cmd = VerifyEmailCommand {
        command_id: cmd_id,
        target: CommandTarget::versioned(f.account_id(), 0),
        region: f.region(),
        code: "123456".to_string(),
    };

    // 2. Act
    let result = f
        .bus()
        .execute::<AccountCommandContext<TransactionManagerStub>, VerifyEmailCommand, ()>(
            f.command_ctx().clone(),
            cmd,
        )
        .await;

    // 3. Assert
    assert!(
        result.is_ok(),
        "L'idempotence technique doit court-circuiter en douceur"
    );

    // L'état ne doit pas avoir bougé par rapport au snapshot initial
    f.assert_account(|acc| {
        assert_eq!(*acc.identity().state(), AccountState::UNVERIFIED);
    })
    .await?;

    f.assert_outbox(0, None);
    Ok(())
}
