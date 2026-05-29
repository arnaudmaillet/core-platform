
use account::commands::settings::ChangePhoneNumberCommand;
use account::context::AccountCommandContext;
use account::events::AccountEvent;
use account::types::AccountState;
use account_test_utils::AccountTestFixture;
use shared_kernel::command::CommandTarget;
use shared_kernel::core::{Error, ErrorCode, Result, Versioned};
use shared_kernel::types::PhoneNumber;
use shared_kernel_test_utils::repositories::TransactionManagerStub;
use uuid::Uuid;

#[tokio::test]
async fn test_change_phone_number_success() -> Result<()> {
    let f = AccountTestFixture::new();
    let old_phone = PhoneNumber::try_new("+33612345678")?;
    let new_phone = PhoneNumber::try_new("+33687654321")?;

    // 1. Arrange : Compte actif avec l'ancien téléphone
    let account = f
        .builder()?
        .with_state(AccountState::ACTIVE)
        .with_phone(old_phone)
        .build()?;

    let version_snapshot = account.version();
    f.account_repo().insert(account);

    let cmd = ChangePhoneNumberCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::new(f.account_id(), f.region(), version_snapshot),
        new_phone: new_phone.clone(),
    };

    // 2. Act
    f.bus()
        .execute::<AccountCommandContext<TransactionManagerStub>, ChangePhoneNumberCommand, ()>(
            f.command_ctx().clone(),
            cmd,
        )
        .await?;

    // 3. Assert
    f.assert_account(|acc| {
        assert_eq!(acc.identity().phone_number(), Some(&new_phone));
        assert_eq!(acc.version(), version_snapshot + 1);
    })
    .await?;

    f.assert_outbox(1, Some(AccountEvent::PHONE_NUMBER_CHANGED));

    Ok(())
}

#[tokio::test]
async fn test_change_phone_technical_idempotency() -> Result<()> {
    let f = AccountTestFixture::new();
    let cmd_id = Uuid::new_v4();
    let requested_phone = PhoneNumber::try_new("+33611223344")?;

    // Arrange : Commande déjà connue par l'infra
    f.idempotency_repo().seed(cmd_id);

    let account = f.builder()?.with_state(AccountState::ACTIVE).build()?;
    let version_snapshot = account.version();
    f.account_repo().insert(account);

    let cmd = ChangePhoneNumberCommand {
        command_id: cmd_id,
        target: CommandTarget::new(f.account_id(), f.region(), version_snapshot),
        new_phone: requested_phone.clone(),
    };

    // Act
    let result = f
        .bus()
        .execute::<AccountCommandContext<TransactionManagerStub>, ChangePhoneNumberCommand, ()>(
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
        assert_ne!(acc.identity().phone_number(), Some(&requested_phone));
        assert_eq!(acc.version(), version_snapshot);
    })
    .await?;

    f.assert_outbox(0, None);
    Ok(())
}

#[tokio::test]
async fn test_change_phone_business_idempotency() -> Result<()> {
    let f = AccountTestFixture::new();
    let phone = PhoneNumber::try_new("+33600000000")?;

    // 1. Arrange : Compte possédant déjà ce numéro
    let account = f
        .builder()?
        .with_state(AccountState::ACTIVE)
        .with_phone(phone.clone())
        .build()?;

    let version_snapshot = account.version();
    f.account_repo().insert(account);

    let cmd = ChangePhoneNumberCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::new(f.account_id(), f.region(), version_snapshot),
        new_phone: phone.clone(),
    };

    // 2. Act
    f.bus()
        .execute::<AccountCommandContext<TransactionManagerStub>, ChangePhoneNumberCommand, ()>(
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

    let account = f.builder()?.with_state(AccountState::ACTIVE).build()?;
    f.account_repo().insert(account);

    // Simulation d'une erreur d'infrastructure lors du commit (Outbox)
    f.outbox_repo().set_error(Error::internal(error_msg));

    let requested_phone = PhoneNumber::try_new("+33611223344")?;
    let cmd = ChangePhoneNumberCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::new(f.account_id(), f.region(), 0),
        new_phone: requested_phone.clone(),
    };

    // 2. Act
    let result = f
        .bus()
        .execute::<AccountCommandContext<TransactionManagerStub>, ChangePhoneNumberCommand, ()>(
            f.command_ctx().clone(),
            cmd,
        )
        .await;

    // 3. Assert
    // On vérifie que l'erreur d'infrastructure est bien remontée au Bus
    match result {
        Err(e) => {
            assert_eq!(e.code, ErrorCode::InternalError);
            assert_eq!(e.message, "An internal server error occurred");
            assert_eq!(e.source(), Some(error_msg));
        }
        Ok(_) => panic!("Should have failed"),
    }

    // NOTE : On ne vérifie pas le rollback ici.
    // Pourquoi ? Parce que f.account_repo() est un InMemoryRepository.
    // Sans une vraie base de données (Postgres), l'annulation atomique
    // des changements en mémoire n'est pas possible via FakeTransaction.
    // Ce test de "vrai rollback" appartient aux tests d'intégration (IT).

    Ok(())
}
