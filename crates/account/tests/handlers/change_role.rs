
use account::commands::lifecycle::ChangeRoleCommand;
use account::context::AccountCommandContext;
use account::events::AccountEvent;
use account::types::AccountRole;
use account_test_utils::AccountTestFixture;
use shared_kernel::command::CommandTarget;
use shared_kernel::core::{Result, Versioned};
use shared_kernel::messaging::EventEmitter;
use shared_kernel::types::AuditReason;
use shared_kernel_test_utils::repositories::TransactionManagerStub;
use uuid::Uuid;

#[tokio::test]
async fn test_change_role_success() -> Result<()> {
    let f = AccountTestFixture::new();
    let account = f.builder()?.build()?;
    let version_snapshot = account.version();

    f.account_repo().insert(account);

    let cmd = ChangeRoleCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.account_id(), f.region(), version_snapshot),
        new_role: AccountRole::MODERATOR,
        reason: AuditReason::try_new("Joined the safety team")?,
    };

    f.bus()
        .execute::<AccountCommandContext<TransactionManagerStub>, ChangeRoleCommand, ()>(f.command_ctx().clone(), cmd)
        .await?;

    f.assert_account(|acc| {
        assert_eq!(acc.governance().role(), AccountRole::MODERATOR);
        assert_eq!(acc.version(), version_snapshot + 1);
    })
    .await?;

    f.assert_outbox(1, Some(AccountEvent::ROLE_CHANGED));

    Ok(())
}

#[tokio::test]
async fn test_change_role_technical_idempotency() -> Result<()> {
    let f = AccountTestFixture::new();
    let cmd_id = Uuid::new_v4();

    f.idempotency_repo().seed(cmd_id);

    let mut account = f.builder()?.build()?;
    let _ = account.change_role(AccountRole::MODERATOR, AuditReason::try_new("init")?);
    account.pull_events();

    let version_snapshot = account.version();
    f.account_repo().insert(account);

    let cmd = ChangeRoleCommand {
        command_id: cmd_id,
        target: CommandTarget::versioned(f.account_id(), f.region(), version_snapshot),
        new_role: AccountRole::MODERATOR,
        reason: AuditReason::try_new("Duplicate promotion")?,
    };

    let result = f
        .bus()
        .execute::<AccountCommandContext<TransactionManagerStub>, ChangeRoleCommand, ()>(f.command_ctx().clone(), cmd)
        .await;

    assert!(
        result.is_ok(),
        "L'idempotence technique doit être transparente (Ok)"
    );

    f.assert_account(|acc| {
        assert_eq!(acc.governance().role(), AccountRole::MODERATOR);
        assert_eq!(acc.version(), version_snapshot);
    })
    .await?;

    f.assert_outbox(0, None);

    Ok(())
}

#[tokio::test]
async fn test_change_role_business_idempotency() -> Result<()> {
    let f = AccountTestFixture::new();
    let mut account = f.builder()?.build()?;

    let _ = account.change_role(AccountRole::MODERATOR, AuditReason::try_new("init")?);
    account.pull_events();

    let version_snapshot = account.version();
    f.account_repo().insert(account);

    let cmd = ChangeRoleCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.account_id(), f.region(), version_snapshot),
        new_role: AccountRole::MODERATOR,
        reason: AuditReason::try_new("Duplicate promotion")?,
    };

    f.bus()
        .execute::<AccountCommandContext<TransactionManagerStub>, ChangeRoleCommand, ()>(f.command_ctx().clone(), cmd)
        .await?;

    f.assert_account(|acc| {
        assert_eq!(acc.governance().role(), AccountRole::MODERATOR);
        assert_eq!(acc.version(), version_snapshot);
    })
    .await?;

    f.assert_outbox(0, None);
    Ok(())
}
