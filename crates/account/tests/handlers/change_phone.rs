// crates/account/tests/handlers/change_phone.rs

use account::commands::settings::ChangePhoneCommand;
use account::context::AccountCommandContext;
use account::events::AccountEvent;
use account::types::AccountState;
use account::types::RegistrationIdentifier;
use account_test_utils::AccountTestFixture;
use shared_kernel::command::CommandTarget;
use shared_kernel::core::{Error, ErrorCode, Result, Versioned};
use shared_kernel::types::Phone;
use shared_kernel_test_utils::repositories::TransactionManagerStub;
use uuid::Uuid;

#[tokio::test]
async fn test_change_phone_success() -> Result<()> {
    let f = AccountTestFixture::new();
    let old_phone = Phone::try_new("+33612345678")?;
    let new_phone = Phone::try_new("+33687654321")?;

    // 1. Arrange : Compte actif avec l'ancien téléphone
    let account = f
        .builder()?
        .with_state(AccountState::ACTIVE)
        .with_phone(old_phone.clone())
        .build()?;

    let version_snapshot = account.version();
    f.account_repo().insert(account);

    f.global_registry()
        .insert_fixture(account::repositories::GlobalIdentityRegistration {
            account_id: f.account_id(),
            region: f.region(),
            sub_id: None,
            identifiers: RegistrationIdentifier::from_phone(old_phone),
            state: AccountState::ACTIVE,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        })
        .await;

    let cmd = ChangePhoneCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.account_id(), f.region(), version_snapshot),
        new_phone: new_phone.clone(),
    };

    // 2. Act
    f.bus()
        .execute::<AccountCommandContext<TransactionManagerStub>, ChangePhoneCommand, ()>(
            f.command_ctx().clone(),
            cmd,
        )
        .await?;

    // 3. Assert
    f.assert_account(|acc| {
        assert_eq!(acc.identity().phone(), Some(&new_phone));
        assert_eq!(acc.version(), version_snapshot + 1);
    })
    .await?;

    f.assert_outbox(1, Some(AccountEvent::PHONE_CHANGED));

    Ok(())
}

#[tokio::test]
async fn test_change_phone_technical_idempotency() -> Result<()> {
    let f = AccountTestFixture::new();
    let cmd_id = Uuid::new_v4();
    let requested_phone = Phone::try_new("+33611223344")?;

    // Arrange : Commande déjà connue par l'infra
    f.idempotency_repo().seed(cmd_id);

    let account = f.builder()?.with_state(AccountState::ACTIVE).build()?;
    let version_snapshot = account.version();
    f.account_repo().insert(account);

    // Note : L'idempotence technique court-circuite le handler avant le global_registry,
    // pas besoin d'insert_fixture ici.

    let cmd = ChangePhoneCommand {
        command_id: cmd_id,
        target: CommandTarget::versioned(f.account_id(), f.region(), version_snapshot),
        new_phone: requested_phone.clone(),
    };

    // Act
    let result = f
        .bus()
        .execute::<AccountCommandContext<TransactionManagerStub>, ChangePhoneCommand, ()>(
            f.command_ctx().clone(),
            cmd,
        )
        .await;

    // Assert
    assert!(
        result.is_ok(),
        "L'idempotence technique doit être transparente (Ok)"
    );

    // VERIFICATION : L'état en base n'a pas bougé
    f.assert_account(|acc| {
        assert_ne!(acc.identity().phone(), Some(&requested_phone));
        assert_eq!(acc.version(), version_snapshot);
    })
    .await?;

    f.assert_outbox(0, None);
    Ok(())
}

#[tokio::test]
async fn test_change_phone_business_idempotency() -> Result<()> {
    let f = AccountTestFixture::new();
    let phone = Phone::try_new("+33600000000")?;

    // 1. Arrange : Compte possédant déjà ce numéro
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
            region: f.region(),
            sub_id: None,
            identifiers: RegistrationIdentifier::from_phone(phone.clone()),
            state: AccountState::ACTIVE,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        })
        .await;

    let cmd = ChangePhoneCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.account_id(), f.region(), version_snapshot),
        new_phone: phone.clone(),
    };

    // 2. Act
    f.bus()
        .execute::<AccountCommandContext<TransactionManagerStub>, ChangePhoneCommand, ()>(
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

#[tokio::test]
async fn test_worst_case_outbox_failure_propagation() -> Result<()> {
    let f = AccountTestFixture::new();
    let error_msg = "Kafka/Outbox DB Error";
    let old_phone = Phone::try_new("+33612345678")?;

    let account = f
        .builder()?
        .with_state(AccountState::ACTIVE)
        .with_phone(old_phone.clone())
        .build()?;
    f.account_repo().insert(account);

    f.global_registry()
        .insert_fixture(account::repositories::GlobalIdentityRegistration {
            account_id: f.account_id(),
            region: f.region(),
            sub_id: None,
            identifiers: RegistrationIdentifier::from_phone(old_phone),
            state: AccountState::ACTIVE,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        })
        .await;

    // Simulation d'une erreur d'infrastructure lors du commit (Outbox)
    f.outbox_repo().set_error(Error::internal(error_msg));

    let requested_phone = Phone::try_new("+33611223344")?;
    let cmd = ChangePhoneCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.account_id(), f.region(), 0),
        new_phone: requested_phone.clone(),
    };

    // 2. Act
    let result = f
        .bus()
        .execute::<AccountCommandContext<TransactionManagerStub>, ChangePhoneCommand, ()>(
            f.command_ctx().clone(),
            cmd,
        )
        .await;

    // 3. Assert
    match result {
        Err(e) => {
            assert_eq!(e.code, ErrorCode::InternalError);
            assert_eq!(e.message, "An internal server error occurred");
            assert_eq!(e.source(), Some(error_msg));
        }
        Ok(_) => panic!("Should have failed"),
    }

    Ok(())
}
