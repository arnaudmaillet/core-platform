// crates/account/tests/handlers/change_email.rs

use account::commands::settings::ChangeEmailCommand;
use account::context::AccountCommandContext;
use account::events::AccountEvent;
use account::types::AccountState;
use account::types::RegistrationIdentifier;
use account_test_utils::AccountTestFixture;
use shared_kernel::command::CommandTarget;
use shared_kernel::core::{Error, ErrorCode, Result, Versioned};
use shared_kernel::types::Email;
use shared_kernel_test_utils::repositories::TransactionManagerStub;
use uuid::Uuid;

#[tokio::test]
async fn test_change_email_success() -> Result<()> {
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
        .insert_fixture(account::repositories::GlobalIdentityRegistration {
            account_id: f.account_id(),
            region: f.region(),
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
region: f.region(),
        new_email: new_email.clone(),
    };

    // 2. Act
    f.bus()
        .execute::<AccountCommandContext<TransactionManagerStub>, ChangeEmailCommand, ()>(
            f.command_ctx().clone(),
            cmd,
        )
        .await?;

    // 3. Assert
    f.assert_account(|acc| {
        assert_eq!(acc.identity().email(), Some(&new_email));
        assert_eq!(acc.version(), version_snapshot + 1);
    })
    .await?;

    f.assert_outbox(1, Some(AccountEvent::EMAIL_CHANGED));

    Ok(())
}

#[tokio::test]
async fn test_change_email_technical_idempotency() -> Result<()> {
    let f = AccountTestFixture::new();
    let cmd_id = Uuid::new_v4();
    let requested_email = Email::try_new("other@test.com")?;
    f.idempotency_repo().seed(cmd_id);

    let account = f.builder()?.with_state(AccountState::ACTIVE).build()?;
    let version_snapshot = account.version();
    f.account_repo().insert(account);

    let cmd = ChangeEmailCommand {
        command_id: cmd_id,
        target: CommandTarget::versioned(f.account_id(), version_snapshot),
region: f.region(),
        new_email: requested_email.clone(),
    };

    // Act
    let result = f
        .bus()
        .execute::<AccountCommandContext<TransactionManagerStub>, ChangeEmailCommand, ()>(
            f.command_ctx().clone(),
            cmd,
        )
        .await;

    // Assert
    assert!(result.is_ok());

    f.assert_account(|acc| {
        assert_ne!(acc.identity().email(), Some(&requested_email));
        assert_eq!(acc.version(), version_snapshot);
    })
    .await?;

    f.assert_outbox(0, None);
    Ok(())
}

#[tokio::test]
async fn test_change_email_business_idempotency() -> Result<()> {
    let f = AccountTestFixture::new();
    let email = Email::try_new("same@test.com")?;

    let account = f
        .builder()?
        .with_state(AccountState::ACTIVE)
        .with_email(email.clone())
        .build()?;

    let version_snapshot = account.version();
    f.account_repo().insert(account);

    f.global_registry()
        .insert_fixture(account::repositories::GlobalIdentityRegistration {
            account_id: f.account_id(),
            region: f.region(),
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
region: f.region(),
        new_email: email,
    };

    // 2. Act
    f.bus()
        .execute::<AccountCommandContext<TransactionManagerStub>, ChangeEmailCommand, ()>(
            f.command_ctx().clone(),
            cmd,
        )
        .await?;

    // 3. Assert
    f.assert_account(|acc| {
        assert_eq!(acc.version(), version_snapshot);
    })
    .await?;

    f.assert_outbox(0, None);
    Ok(())
}

#[tokio::test]
async fn test_change_email_forbidden_when_restricted() -> Result<()> {
    let f = AccountTestFixture::new();
    let requested_email = Email::try_new("new@test.com")?;
    let old_email = Email::try_new("old@test.com")?;

    // Arrange : Un banni ne peut pas modifier ses réglages
    let account = f
        .builder()?
        .with_state(AccountState::BANNED)
        .with_email(old_email.clone())
        .build()?;

    let version_snapshot = account.version();
    f.account_repo().insert(account);

    f.global_registry()
        .insert_fixture(account::repositories::GlobalIdentityRegistration {
            account_id: f.account_id(),
            region: f.region(),
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
region: f.region(),
        new_email: requested_email.clone(),
    };

    // Act
    let result = f
        .bus()
        .execute::<AccountCommandContext<TransactionManagerStub>, ChangeEmailCommand, ()>(
            f.command_ctx().clone(),
            cmd,
        )
        .await;

    match result {
        Err(e) => {
            assert_eq!(e.code, ErrorCode::Forbidden);
        }
        Ok(_) => panic!("Should have failed: a banned account cannot change its email"),
    }

    f.assert_account(|acc| {
        assert_ne!(acc.identity().email(), Some(&requested_email));
        assert_eq!(acc.version(), version_snapshot);
    })
    .await?;

    Ok(())
}

#[tokio::test]
async fn test_change_email_succeeds_after_retry() -> Result<()> {
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
        .insert_fixture(account::repositories::GlobalIdentityRegistration {
            account_id: f.account_id(),
            region: f.region(),
            sub_id: None,
            identifiers: RegistrationIdentifier::from_email(old_email),
            state: AccountState::ACTIVE,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        })
        .await;

    // 1. Arrange : Simulation d'une erreur OCC
    f.account_repo()
        .set_error_once(Error::concurrency_conflict("Version mismatch"));

    let cmd = ChangeEmailCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.account_id(), version_snapshot),
region: f.region(),
        new_email: requested_email.clone(),
    };

    // 2. Act
    let result = f
        .bus()
        .execute::<AccountCommandContext<TransactionManagerStub>, ChangeEmailCommand, ()>(
            f.command_ctx().clone(),
            cmd,
        )
        .await;

    // 3. Assert
    assert!(result.is_ok());

    f.assert_account(|acc| {
        assert_eq!(acc.identity().email(), Some(&requested_email));
        assert_eq!(acc.version(), version_snapshot + 1);
    })
    .await?;

    f.assert_outbox(1, Some(AccountEvent::EMAIL_CHANGED));

    Ok(())
}
