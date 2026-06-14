// crates/account/src/application/use_cases/lifecycle/change_role/change_role_use_case_test.rs

use account::commands::lifecycle::ChangeRoleCommand;
use account::context::AccountCommandCtx;
use account::events::AccountEvent;
use account::types::AccountRole;
use account_test_utils::asserts::AccountRepositoryAsserts;

use account_test_utils::AccountTestFixture;
use shared_kernel::command::CommandTarget;
use shared_kernel::core::{Result, Versioned};
use shared_kernel::idempotency::IdempotencyRepository;
use shared_kernel::messaging::EventEmitter;
use shared_kernel::types::AuditReason;
use uuid::Uuid;

#[tokio::test]
async fn test_change_role_success() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();
    let account = f.builder()?.build()?;
    let version_snapshot = account.version();
    f.account_repo().insert(account);

    let reason = AuditReason::try_new("Joined the safety team")?;
    let cmd = ChangeRoleCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.account_id(), version_snapshot),
        region: f.server_region(),
        new_role: AccountRole::MODERATOR,
        reason: reason.clone(),
    };

    // Act
    f.bus()
        .execute::<AccountCommandCtx, ChangeRoleCommand, ()>(f.command_ctx().clone(), cmd)
        .await?;

    // Assert
    f.account_assertions()
        .assert_account_state(f.account_id(), |acc| {
            assert_eq!(acc.governance().role(), AccountRole::MODERATOR);
            assert_eq!(acc.version(), version_snapshot + 1);
        })
        .await;

    f.assert_outbox(1, Some(AccountEvent::ROLE_CHANGED));

    Ok(())
}

#[tokio::test]
async fn test_change_role_technical_idempotency() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();
    let cmd_id = Uuid::new_v4();

    // On simule une commande déjà interceptée par la barrière d'idempotence technique
    f.idempotency_repo().save(None, &cmd_id).await?;

    let mut account = f.builder()?.build()?;
    let _ = account.change_role(AccountRole::MODERATOR, AuditReason::try_new("init")?);
    account.pull_events(); // On vide l'outbox locale du setup

    let version_snapshot = account.version();
    f.account_repo().insert(account);

    let cmd = ChangeRoleCommand {
        command_id: cmd_id,
        target: CommandTarget::versioned(f.account_id(), version_snapshot),
        region: f.server_region(),
        new_role: AccountRole::MODERATOR,
        reason: AuditReason::try_new("Duplicate promotion")?,
    };

    // Act
    let result = f
        .bus()
        .execute::<AccountCommandCtx, ChangeRoleCommand, ()>(f.command_ctx().clone(), cmd)
        .await;

    // Assert
    assert!(
        result.is_ok(),
        "L'idempotence technique doit court-circuiter de façon transparente (Ok)"
    );

    // L'état en base n'a pas bougé
    f.account_assertions()
        .assert_account_state(f.account_id(), |acc| {
            assert_eq!(acc.governance().role(), AccountRole::MODERATOR);
            assert_eq!(acc.version(), version_snapshot);
        })
        .await;

    // Aucun événement dupliqué n'est poussé dans le journal Outbox
    f.account_assertions()
        .assert_no_events_for(f.account_id())
        .await;

    Ok(())
}

#[tokio::test]
async fn test_change_role_business_idempotency() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();
    let mut account = f.builder()?.build()?;

    // Idempotence métier : Le compte possède déjà le rôle demandé
    let _ = account.change_role(AccountRole::MODERATOR, AuditReason::try_new("init")?);
    account.pull_events();

    let version_snapshot = account.version();
    f.account_repo().insert(account);

    let cmd = ChangeRoleCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.account_id(), version_snapshot),
        region: f.server_region(),
        new_role: AccountRole::MODERATOR,
        reason: AuditReason::try_new("Duplicate promotion")?,
    };

    // Act
    f.bus()
        .execute::<AccountCommandCtx, ChangeRoleCommand, ()>(f.command_ctx().clone(), cmd)
        .await?;

    // Assert
    // L'exécution doit réussir mais ne rien changer (No-Op transactionnel)
    f.account_assertions()
        .assert_account_state(f.account_id(), |acc| {
            assert_eq!(acc.governance().role(), AccountRole::MODERATOR);
            assert_eq!(acc.version(), version_snapshot);
        })
        .await;

    // Aucun événement métier produit puisque l'état est inchangé
    f.account_assertions()
        .assert_no_events_for(f.account_id())
        .await;

    Ok(())
}
