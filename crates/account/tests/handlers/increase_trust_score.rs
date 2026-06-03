
use account::commands::moderation::IncreaseTrustScoreCommand;
use account::context::AccountCommandContext;
use account::events::AccountEvent;
use account::types::{AccountState, TrustAmount, TrustScore};
use account_test_utils::AccountTestFixture;
use shared_kernel::command::CommandTarget;
use shared_kernel::core::{Result, Versioned};
use shared_kernel::types::AuditReason;
use shared_kernel_test_utils::repositories::TransactionManagerStub;
use uuid::Uuid;

#[tokio::test]
async fn test_increase_trust_score_success() -> Result<()> {
    let f = AccountTestFixture::new();

    // 1. Arrange : Score initial à 50
    let account = f
        .builder()?
        .with_state(AccountState::ACTIVE)
        .with_trust_score(TrustScore::from_raw(50))
        .build()?;

    let version_snapshot = account.version();
    f.account_repo().insert(account);

    let cmd = IncreaseTrustScoreCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.account_id(), f.region(), version_snapshot),
        amount: TrustAmount::try_from(20)?, // 50 + 20 = 70
        reason: AuditReason::try_new("Good behavior")?,
    };

    // 2. Act
    f.bus()
        .execute::<AccountCommandContext<TransactionManagerStub>, IncreaseTrustScoreCommand, ()>(
            f.command_ctx().clone(),
            cmd,
        )
        .await?;

    // 3. Assert
    f.assert_account(|acc| {
        assert_eq!(acc.governance().trust_score().value(), 70);
        assert_eq!(acc.version(), version_snapshot + 1);
    })
    .await?;

    f.assert_outbox(1, Some(AccountEvent::TRUST_SCORE_REWARDED));

    Ok(())
}

#[tokio::test]
async fn test_increase_trust_score_cap_at_one_hundred() -> Result<()> {
    let f = AccountTestFixture::new();

    // 1. Arrange : Score à 90
    let account = f
        .builder()?
        .with_state(AccountState::ACTIVE)
        .with_trust_score(TrustScore::from_raw(50))
        .build()?;

    let version_snapshot = account.version();
    f.account_repo().insert(account);

    let cmd = IncreaseTrustScoreCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.account_id(), f.region(), version_snapshot),
        amount: TrustAmount::try_from(50)?, // 90 + 50 -> Saturé à 100
        reason: AuditReason::try_new("High activity")?,
    };

    // 2. Act
    f.bus()
        .execute::<AccountCommandContext<TransactionManagerStub>, IncreaseTrustScoreCommand, ()>(
            f.command_ctx().clone(),
            cmd,
        )
        .await?;

    // 3. Assert
    f.assert_account(|acc| {
        assert_eq!(acc.governance().trust_score().value(), 100);
        assert_eq!(acc.version(), version_snapshot + 1);
    })
    .await?;

    Ok(())
}

#[tokio::test]
async fn test_increase_trust_score_technical_idempotency() -> Result<()> {
    let f = AccountTestFixture::new();
    let cmd_id = Uuid::new_v4();

    f.idempotency_repo().seed(cmd_id);

    let account = f.builder()?.with_state(AccountState::ACTIVE).build()?;
    let version_snapshot = account.version();
    f.account_repo().insert(account);

    let cmd = IncreaseTrustScoreCommand {
        command_id: cmd_id,
        target: CommandTarget::versioned(f.account_id(), f.region(), version_snapshot),
        amount: TrustAmount::try_from(10)?,
        reason: AuditReason::try_new("Duplicate")?,
    };

    let result = f
        .bus()
        .execute::<AccountCommandContext<TransactionManagerStub>, IncreaseTrustScoreCommand, ()>(
            f.command_ctx().clone(),
            cmd,
        )
        .await;

    f.assert_account(|acc| {
        assert_eq!(acc.governance().trust_score().value(), 100);
        assert_eq!(acc.version(), version_snapshot);
    })
    .await?;

    assert!(
        result.is_ok(),
        "L'idempotence technique doit être transparente (Ok)"
    );
    f.assert_outbox(0, None);

    f.assert_account(|acc| {
        assert_eq!(
            acc.version(),
            version_snapshot,
            "La version ne doit pas avoir augmenté"
        );
    })
    .await?;
    Ok(())
}

#[tokio::test]
async fn test_increase_trust_score_business_idempotency_at_max() -> Result<()> {
    let f = AccountTestFixture::new();

    // Arrange : Déjà au maximum (100)
    let account = f
        .builder()?
        .with_state(AccountState::ACTIVE)
        .with_trust_score(TrustScore::from_raw(TrustScore::MAX))
        .build()?;

    let version_snapshot = account.version();
    f.account_repo().insert(account);

    let cmd = IncreaseTrustScoreCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.account_id(), f.region(), version_snapshot),
        amount: TrustAmount::try_from(10)?,
        reason: AuditReason::try_new("Should do nothing")?,
    };

    // 2. Act
    f.bus()
        .execute::<AccountCommandContext<TransactionManagerStub>, IncreaseTrustScoreCommand, ()>(
            f.command_ctx().clone(),
            cmd,
        )
        .await?;

    // 3. Assert
    f.assert_account(|acc| {
        assert_eq!(acc.governance().trust_score().value(), 100);
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
