// crates/account/src/application/use_cases/lifecycle/activate/activate_use_case_test.rs

use account::commands::lifecycle::ActivateCommand;
use account::context::AccountCommandCtx;
use account::events::AccountEvent;
use account::types::AccountState;
use account_test_utils::asserts::AccountRepositoryAsserts;

use account_test_utils::AccountTestFixture;
use shared_kernel::command::CommandTarget;
use shared_kernel::core::{ErrorCode, Result, Versioned};
use shared_kernel::idempotency::IdempotencyRepository;
use shared_kernel::messaging::EventEmitter;
use shared_kernel::types::AuditReason;
use uuid::Uuid;

#[tokio::test]
async fn test_activate_account_success() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();

    let account = f.builder()?.with_state(AccountState::DEACTIVATED).build()?;

    let version_snapshot = account.version();
    let target_account_id = f.account_id();
    f.account_repo().insert(account);

    let cmd = ActivateCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(target_account_id, version_snapshot),
        region: f.server_region(),
    };

    // Act
    f.bus()
        .execute::<AccountCommandCtx, ActivateCommand, ()>(f.command_ctx().clone(), cmd)
        .await?;

    // Assert
    f.account_assertions()
        .assert_account_state(target_account_id, |acc| {
            assert_eq!(*acc.identity().state(), AccountState::ACTIVE);
            assert_eq!(acc.version(), version_snapshot + 1);
        })
        .await;

    f.assert_outbox(1, Some(AccountEvent::ACTIVATED));

    Ok(())
}

#[tokio::test]
async fn test_activate_technical_idempotency() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();
    let cmd_id = Uuid::new_v4();

    // On simule une commande déjà traitée et validée par le premier rideau d'idempotence
    f.idempotency_repo().save(None, &cmd_id).await?;

    let mut account = f.builder()?.build()?;
    account.deactivate(None)?;
    account.pull_events();

    let version_snapshot = account.version();
    f.account_repo().insert(account);

    let cmd = ActivateCommand {
        command_id: cmd_id,
        target: CommandTarget::versioned(f.account_id(), version_snapshot),
        region: f.server_region(),
    };

    // Act
    let result = f
        .bus()
        .execute::<AccountCommandCtx, ActivateCommand, ()>(f.command_ctx().clone(), cmd)
        .await;

    // Assert
    assert!(
        result.is_ok(),
        "L'idempotence technique doit court-circuiter de façon transparente (Ok)"
    );

    // L'agrégat n'a subi aucune mutation ni changement de version
    f.account_assertions()
        .assert_account_state(f.account_id(), |acc| {
            assert_eq!(*acc.identity().state(), AccountState::DEACTIVATED);
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
async fn test_activate_business_idempotency() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();

    // Le compte est déjà actif (Business Idempotency)
    let mut account = f.builder()?.build()?;
    account.activate()?;
    account.pull_events();

    let version_snapshot = account.version();
    f.account_repo().insert(account);

    let cmd = ActivateCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.account_id(), version_snapshot),
        region: f.server_region(),
    };

    // Act
    f.bus()
        .execute::<AccountCommandCtx, ActivateCommand, ()>(f.command_ctx().clone(), cmd)
        .await?;

    // Assert
    // Pas de modification d'état ni d'incrément de version (No-Op transactionnel)
    f.account_assertions()
        .assert_account_state(f.account_id(), |acc| {
            assert_eq!(*acc.identity().state(), AccountState::ACTIVE);
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
async fn test_activate_forbidden_if_banned() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();

    let mut account = f.builder()?.build()?;
    account.ban(AuditReason::try_new("Violation stricte des CGU")?)?;
    account.pull_events();

    let version_snapshot = account.version();
    f.account_repo().insert(account);

    let cmd = ActivateCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.account_id(), version_snapshot),
        region: f.server_region(),
    };

    // Act
    let result = f
        .bus()
        .execute::<AccountCommandCtx, ActivateCommand, ()>(f.command_ctx().clone(), cmd)
        .await;

    // Assert
    match result {
        Err(e) => {
            assert_eq!(
                e.code,
                ErrorCode::Forbidden,
                "L'action aurait dû lever une interdiction (Forbidden)"
            );
        }
        Ok(_) => {
            panic!("Le cas d'usage aurait dû échouer : un compte banni ne peut pas être réactivé")
        }
    }

    // Sécurité de l'état : l'invariant a tenu bon, aucune écriture
    f.account_assertions()
        .assert_account_state(f.account_id(), |acc| {
            assert_eq!(*acc.identity().state(), AccountState::BANNED);
            assert_eq!(acc.version(), version_snapshot);
        })
        .await;

    f.account_assertions()
        .assert_no_events_for(f.account_id())
        .await;

    Ok(())
}
