use account::commands::access_management::VerifyPhoneCommand;
use account::context::AccountCommandContext;
use account::events::AccountEvent;
use account::repositories::{GlobalIdentityRegistration, OtpRepository};
use account::types::{AccountState, RegistrationIdentifier};
use account_test_utils::AccountTestFixture;
use shared_kernel::command::CommandTarget;
use shared_kernel::core::{ErrorCode, Result, Versioned};
use shared_kernel::types::Phone;
use shared_kernel_test_utils::repositories::TransactionManagerStub;
use uuid::Uuid;

#[tokio::test]
async fn test_verify_phone_success() -> Result<()> {
    let f = AccountTestFixture::new();
    let code = "123456";
    let phone = Phone::try_new("+33612345678")?;

    // 1. Arrange : Un compte avec téléphone non vérifié au niveau régional
    let account = f
        .builder()?
        .with_state(AccountState::UNVERIFIED)
        .with_phone(phone.clone()) // On s'assure que l'agrégat porte ce numéro
        .build()?;
    let version_snapshot = account.version();
    f.account_repo().insert(account.clone());

    // On peuple le registre global en état UNVERIFIED avec ce numéro de téléphone
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

    // On simule la présence du code OTP Téléphone (purpose "phone") généré au préalable
    f.otp_repo().seed_code(f.account_id(), "phone", code);

    let cmd = VerifyPhoneCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.account_id(), f.region(), version_snapshot),
        code: code.to_string(),
    };

    // 2. Act : Exécution du handler via le bus
    f.bus()
        .execute::<AccountCommandContext<TransactionManagerStub>, VerifyPhoneCommand, ()>(
            f.command_ctx().clone(),
            cmd,
        )
        .await?;

    // 3. Assert : L'agrégat régional doit être ACTIVE et le timestamp peuplé
    f.assert_account(|acc| {
        assert_eq!(*acc.identity().state(), AccountState::ACTIVE);
        assert!(acc.identity().phone_verified_at().is_some());
        assert_eq!(acc.version(), version_snapshot + 1); // OCC incrémenté
    })
    .await?;

    // 4. Assert : L'OTP doit avoir été nettoyé du cache Redis
    let cached_code: Option<String> = f.otp_repo().get_code(&f.account_id(), "phone").await?;
    assert!(
        cached_code.is_none(),
        "L'OTP téléphone aurait dû être supprimé"
    );

    // 5. Assert : Événement Outbox Kafka envoyé
    f.assert_outbox(1, Some(AccountEvent::PHONE_VERIFIED));

    Ok(())
}

#[tokio::test]
async fn test_verify_phone_fails_if_otp_invalid() -> Result<()> {
    let f = AccountTestFixture::new();
    let phone = Phone::try_new("+33612345678")?;

    // Arrange
    let account = f
        .builder()?
        .with_state(AccountState::UNVERIFIED)
        .with_phone(phone)
        .build()?;
    f.account_repo().insert(account);
    f.otp_repo().seed_code(f.account_id(), "phone", "123456");

    let cmd = VerifyPhoneCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.account_id(), f.region(), 0),
        code: "654321".to_string(), // Mauvais code fourni
    };

    // 2. Act
    let result = f
        .bus()
        .execute::<AccountCommandContext<TransactionManagerStub>, VerifyPhoneCommand, ()>(
            f.command_ctx().clone(),
            cmd,
        )
        .await;

    // 3. Assert
    match result {
        Err(e) => assert_eq!(e.code, ErrorCode::ValidationFailed),
        Ok(_) => panic!("Should have failed: invalid OTP code"),
    }

    // L'agrégat ne doit pas avoir changé
    f.assert_account(|acc| {
        assert_eq!(*acc.identity().state(), AccountState::UNVERIFIED);
    })
    .await?;

    // Le code doit rester disponible pour une nouvelle tentative
    let cached_code = f.otp_repo().get_code(&f.account_id(), "phone").await?;
    assert!(cached_code.is_some());
    f.assert_outbox(0, None);

    Ok(())
}

#[tokio::test]
async fn test_verify_phone_fails_if_otp_expired() -> Result<()> {
    let f = AccountTestFixture::new();
    let phone = Phone::try_new("+33612345678")?;

    // Arrange : Aucun code provisionné dans le stub (simule l'expiration Redis)
    let account = f
        .builder()?
        .with_state(AccountState::UNVERIFIED)
        .with_phone(phone)
        .build()?;
    f.account_repo().insert(account);

    let cmd = VerifyPhoneCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.account_id(), f.region(), 0),
        code: "123456".to_string(),
    };

    // 2. Act
    let result = f
        .bus()
        .execute::<AccountCommandContext<TransactionManagerStub>, VerifyPhoneCommand, ()>(
            f.command_ctx().clone(),
            cmd,
        )
        .await;

    // 3. Assert
    match result {
        Err(e) => assert_eq!(e.code, ErrorCode::ValidationFailed),
        Ok(_) => panic!("Should have failed: OTP expired or non-existent"),
    }

    f.assert_outbox(0, None);
    Ok(())
}

#[tokio::test]
async fn test_verify_phone_technical_idempotency() -> Result<()> {
    let f = AccountTestFixture::new();
    let cmd_id = Uuid::new_v4();
    let phone = Phone::try_new("+33612345678")?;

    // Arrange : Commande déjà enregistrée dans la table d'idempotence technique
    f.idempotency_repo().seed(cmd_id);

    let account = f
        .builder()?
        .with_state(AccountState::UNVERIFIED)
        .with_phone(phone)
        .build()?;
    f.account_repo().insert(account);
    f.otp_repo().seed_code(f.account_id(), "phone", "123456");

    let cmd = VerifyPhoneCommand {
        command_id: cmd_id,
        target: CommandTarget::versioned(f.account_id(), f.region(), 0),
        code: "123456".to_string(),
    };

    // 2. Act
    let result = f
        .bus()
        .execute::<AccountCommandContext<TransactionManagerStub>, VerifyPhoneCommand, ()>(
            f.command_ctx().clone(),
            cmd,
        )
        .await;

    // 3. Assert
    assert!(
        result.is_ok(),
        "L'idempotence doit retourner Ok sans ré-exécuter le métier"
    );

    f.assert_account(|acc| {
        assert_eq!(*acc.identity().state(), AccountState::UNVERIFIED);
        assert!(acc.identity().phone_verified_at().is_none());
    })
    .await?;

    f.assert_outbox(0, None);
    Ok(())
}
