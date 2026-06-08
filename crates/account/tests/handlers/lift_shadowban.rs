
use account::commands::moderation::LiftShadowbanCommand;
use account::context::AccountCommandContext;
use account::events::AccountEvent;
use account::types::AccountState;
use account_test_utils::AccountTestFixture;
use shared_kernel::command::CommandTarget;
use shared_kernel::core::{Result, Versioned};
use shared_kernel::types::AuditReason;
use shared_kernel_test_utils::repositories::TransactionManagerStub;
use uuid::Uuid;

#[tokio::test]
async fn test_lift_shadowban_success() -> Result<()> {
    let f = AccountTestFixture::new();

    // 1. Arrange : Un compte banni est automatiquement shadowbanné par notre builder
    let account = f.builder()?.with_state(AccountState::BANNED).build()?;

    let version_snapshot = account.version();
    f.account_repo().insert(account);

    let cmd = LiftShadowbanCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.account_id(), version_snapshot),
region: f.region(),
        reason: AuditReason::try_new("Appeal accepted")?,
    };

    // 2. Act
    f.bus()
        .execute::<AccountCommandContext<TransactionManagerStub>, LiftShadowbanCommand, ()>(f.command_ctx().clone(), cmd)
        .await?;

    // 3. Assert
    f.assert_account(|acc| {
        assert!(
            !acc.governance().is_shadowbanned(),
            "Le shadowban doit être levé"
        );
        assert_eq!(acc.version(), version_snapshot + 1);
    })
    .await?;

    f.assert_outbox(1, Some(AccountEvent::SHADOWBAN_UPDATED));

    Ok(())
}

#[tokio::test]
async fn test_lift_shadowban_technical_idempotency() -> Result<()> {
    let f = AccountTestFixture::new();
    let cmd_id = Uuid::new_v4();

    // Arrange
    f.idempotency_repo().seed(cmd_id);

    let account = f.builder()?.with_state(AccountState::BANNED).build()?;
    let version_snapshot = account.version();
    f.account_repo().insert(account);

    let cmd = LiftShadowbanCommand {
        command_id: cmd_id,
        target: CommandTarget::versioned(f.account_id(), version_snapshot),
region: f.region(),
        reason: AuditReason::try_new("Duplicate call")?,
    };

    // Act
    let result = f
        .bus()
        .execute::<AccountCommandContext<TransactionManagerStub>, LiftShadowbanCommand, ()>(f.command_ctx().clone(), cmd)
        .await;

    // Assert
    assert!(
        result.is_ok(),
        "L'idempotence technique doit être transparente (Ok)"
    );
    f.assert_outbox(0, None);

    Ok(())
}

#[tokio::test]
async fn test_lift_shadowban_business_idempotency() -> Result<()> {
    let f = AccountTestFixture::new();

    // 1. Arrange : Compte déjà sain (Shadowban = false par défaut)
    let account = f.builder()?.with_state(AccountState::ACTIVE).build()?;
    let version_snapshot = account.version();
    f.account_repo().insert(account);

    let cmd = LiftShadowbanCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.account_id(), version_snapshot),
region: f.region(),
        reason: AuditReason::try_new("Accidental click")?,
    };

    // 2. Act
    f.bus()
        .execute::<AccountCommandContext<TransactionManagerStub>, LiftShadowbanCommand, ()>(f.command_ctx().clone(), cmd)
        .await?;

    // 3. Assert
    f.assert_account(|acc| {
        assert!(!acc.governance().is_shadowbanned());
        assert_eq!(
            acc.version(),
            version_snapshot,
            "La version ne doit pas augmenter si aucun changement"
        );
    })
    .await?;

    f.assert_outbox(0, None);

    Ok(())
}
