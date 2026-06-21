// crates/account/src/application/access_management/register/register_use_case_test.rs

use account_old::commands::access_management::RegisterCommand;
use account_old::context::AccountCommandCtx;
use account_old::events::AccountEvent;
use account_old::repositories::GlobalIdentityRegistration;
use account_old::types::{AccountState, IpAddr, Locale, RegistrationIdentifier};
use account_test_utils::asserts::AccountRepositoryAsserts;

use account_test_utils::AccountTestFixture;
use shared_kernel::command::CommandTarget;
use shared_kernel::core::{Error, ErrorCode, Result, Versioned};
use shared_kernel::types::{Email, SubId};
use uuid::Uuid;

#[tokio::test]
async fn test_register_success() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();
    let email = Email::try_new("new-user@example.com")?;
    let ext_id = SubId::from_raw("keycloak|12345");
    let ip = IpAddr::try_new("127.0.0.1")?;

    let expected_account_id = f.account_id();
    let target = CommandTarget::stateless(expected_account_id);

    let cmd = RegisterCommand {
        command_id: Uuid::new_v4(),
        target,
        region: f.server_region(),
        sub_id: Some(ext_id.clone()),
        identifier: RegistrationIdentifier::from_email(email.clone()),
        locale: Locale::try_new("en-US")?,
        ip_addr: ip.clone(),
    };

    // Act
    let result = f
        .bus()
        .execute::<AccountCommandCtx, RegisterCommand, ()>(f.command_ctx().clone(), cmd)
        .await;

    // Assert
    assert!(
        result.is_ok(),
        "Le register devrait réussir : {:?}",
        result.err()
    );

    f.account_assertions()
        .assert_account_state(expected_account_id, |acc| {
            assert_eq!(acc.identity().email(), Some(&email));
            assert_eq!(acc.identity().sub_id(), Some(&ext_id));
            assert_eq!(*acc.identity().state(), AccountState::ACTIVE);
            assert_eq!(acc.governance().last_ip_addr(), Some(&ip));

            // v1 car INITIAL_VERSION (0) + 1 (register call)
            assert_eq!(acc.version(), 1);
        })
        .await;

    f.assert_outbox(1, Some(AccountEvent::REGISTERED));

    Ok(())
}

#[tokio::test]
async fn test_register_fails_if_sub_id_already_exists() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();
    let existing_ext_id = SubId::from_raw("duplicate_id");
    let email = Email::try_new("new@test.com")?;

    let registration = GlobalIdentityRegistration {
        account_id: f.account_id(),
        region: f.server_region(),
        sub_id: Some(existing_ext_id.clone()),
        identifiers: RegistrationIdentifier::from_email(Email::try_new("already_exists@test.com")?),
        state: AccountState::UNVERIFIED,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    f.global_registry().insert_fixture(registration).await;

    let target = CommandTarget::stateless(f.account_id());

    let cmd = RegisterCommand {
        command_id: Uuid::new_v4(),
        target,
        region: f.server_region(),
        sub_id: Some(existing_ext_id),
        identifier: RegistrationIdentifier::from_email(email),
        locale: Locale::try_new("en-US")?,
        ip_addr: IpAddr::try_new("127.0.0.1")?,
    };

    // Act
    let result = f
        .bus()
        .execute::<AccountCommandCtx, RegisterCommand, ()>(f.command_ctx().clone(), cmd)
        .await;

    // Assert
    match result {
        Err(e) => {
            assert_eq!(e.code, ErrorCode::ValidationFailed);
            assert!(
                e.message.contains("sub_id"),
                "L'erreur aurait dû cibler le sub_id, message reçu: {}",
                e.message
            );
        }
        _ => panic!("Devrait échouer car le sub_id existe déjà au niveau du Global Registry"),
    }

    // Aucun événement produit suite à l'échec de validation
    f.account_assertions()
        .assert_no_events_for(f.account_id())
        .await;

    Ok(())
}

#[tokio::test]
async fn test_register_atomic_rollback_on_outbox_failure() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();
    let error_msg = "Outbox DB Crash";
    let email = Email::try_new("atomic@test.com")?;

    // On force une erreur d'infrastructure dans l'outbox stub pour déclencher le rollback
    f.outbox_repo().set_error(Error::internal(error_msg));
    let target = CommandTarget::stateless(f.account_id());

    let cmd = RegisterCommand {
        command_id: Uuid::new_v4(),
        target,
        region: f.server_region(),
        sub_id: Some(SubId::from_raw("atomic_ext")),
        identifier: RegistrationIdentifier::from_email(email),
        locale: Locale::try_new("en-US")?,
        ip_addr: IpAddr::try_new("127.0.0.1")?,
    };

    // Act
    let result = f
        .bus()
        .execute::<AccountCommandCtx, RegisterCommand, ()>(f.command_ctx().clone(), cmd)
        .await;

    // Assert
    match result {
        Err(e) => {
            assert_eq!(e.code, ErrorCode::InternalError);
            assert_eq!(e.message, "An internal server error occurred");
            assert_eq!(e.source(), Some(error_msg));
        }
        Ok(_) => {
            panic!("Le use case aurait dû échouer suite au Rollback atomique de la transaction")
        }
    }

    // Vérification cruciale de l'atomicité : L'agrégat ne doit pas exister dans le store régional
    // suite au rollback provoqué par l'échec de l'outbox.
    f.account_assertions()
        .assert_no_events_for(f.account_id())
        .await;

    Ok(())
}
