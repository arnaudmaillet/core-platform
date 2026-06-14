// crates/account/src/application/use_cases/moderation/increase_trust_score/increase_trust_score_use_case_test.rs

use account::commands::moderation::IncreaseTrustScoreCommand;
use account::context::AccountCommandCtx;
use account::events::AccountEvent;
use account::types::{AccountState, TrustAmount, TrustScore};
use account_test_utils::asserts::AccountRepositoryAsserts;

use account_test_utils::AccountTestFixture;
use shared_kernel::command::CommandTarget;
use shared_kernel::core::{Result, Versioned};
use shared_kernel::idempotency::IdempotencyRepository;
use shared_kernel::types::AuditReason;
use uuid::Uuid;

#[tokio::test]
async fn test_increase_trust_score_success() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();

    // 1. Score initial configuré à 50
    let account = f
        .builder()?
        .with_state(AccountState::ACTIVE)
        .with_trust_score(TrustScore::from_raw(50))
        .build()?;

    let version_snapshot = account.version();
    f.account_repo().insert(account);

    let reason = AuditReason::try_new("Good behavior")?;
    let cmd = IncreaseTrustScoreCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.account_id(), version_snapshot),
        region: f.server_region(),
        amount: TrustAmount::try_from(20)?, // 50 + 20 = 70
        reason: reason.clone(),
    };

    // Act
    f.bus()
        .execute::<AccountCommandCtx, IncreaseTrustScoreCommand, ()>(f.command_ctx().clone(), cmd)
        .await?;

    // Assert
    f.account_assertions()
        .assert_account_state(f.account_id(), |acc| {
            assert_eq!(acc.governance().trust_score().value(), 70);
            assert_eq!(acc.version(), version_snapshot + 1);
        })
        .await;

    f.assert_outbox(1, Some(AccountEvent::TRUST_SCORE_REWARDED));

    Ok(())
}

#[tokio::test]
async fn test_increase_trust_score_cap_at_one_hundred() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();

    // 1. Fix de l'arrangement : On initialise bien le score à 90 pour valider le plafond (Cap)
    let account = f
        .builder()?
        .with_state(AccountState::ACTIVE)
        .with_trust_score(TrustScore::from_raw(90))
        .build()?;

    let version_snapshot = account.version();
    f.account_repo().insert(account);

    let cmd = IncreaseTrustScoreCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.account_id(), version_snapshot),
        region: f.server_region(),
        amount: TrustAmount::try_from(50)?, // 90 + 50 -> Cap à 100 au niveau du domaine
        reason: AuditReason::try_new("High activity")?,
    };

    // Act
    f.bus()
        .execute::<AccountCommandCtx, IncreaseTrustScoreCommand, ()>(f.command_ctx().clone(), cmd)
        .await?;

    // Assert
    f.account_assertions()
        .assert_account_state(f.account_id(), |acc| {
            assert_eq!(acc.governance().trust_score().value(), 100);
            assert_eq!(acc.version(), version_snapshot + 1);
        })
        .await;

    Ok(())
}

#[tokio::test]
async fn test_increase_trust_score_technical_idempotency() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();
    let cmd_id = Uuid::new_v4();

    // On simule une interception transparente par la barrière d'idempotence technique
    f.idempotency_repo().save(None, &cmd_id).await?;

    // Par défaut, le builder met le score au max (100)
    let account = f.builder()?.with_state(AccountState::ACTIVE).build()?;
    let version_snapshot = account.version();
    f.account_repo().insert(account);

    let cmd = IncreaseTrustScoreCommand {
        command_id: cmd_id,
        target: CommandTarget::versioned(f.account_id(), version_snapshot),
        region: f.server_region(),
        amount: TrustAmount::try_from(10)?,
        reason: AuditReason::try_new("Duplicate")?,
    };

    // Act
    let result = f
        .bus()
        .execute::<AccountCommandCtx, IncreaseTrustScoreCommand, ()>(f.command_ctx().clone(), cmd)
        .await;

    // Assert
    assert!(
        result.is_ok(),
        "L'idempotence technique doit court-circuiter de façon transparente (Ok)"
    );

    // L'état en mémoire ou stub n'a subi aucune écriture additionnelle ni mutation
    f.account_assertions()
        .assert_account_state(f.account_id(), |acc| {
            assert_eq!(
                acc.version(),
                version_snapshot,
                "La version ne doit pas changer"
            );
        })
        .await;

    // L'outbox locale reste intacte
    f.account_assertions()
        .assert_no_events_for(f.account_id())
        .await;

    Ok(())
}

#[tokio::test]
async fn test_increase_trust_score_business_idempotency_at_max() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();

    // Idempotence métier : Le compte est déjà configuré à la limite maximale autorisée
    let account = f
        .builder()?
        .with_state(AccountState::ACTIVE)
        .with_trust_score(TrustScore::from_raw(TrustScore::MAX))
        .build()?;

    let version_snapshot = account.version();
    f.account_repo().insert(account);

    let cmd = IncreaseTrustScoreCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.account_id(), version_snapshot),
        region: f.server_region(),
        amount: TrustAmount::try_from(10)?,
        reason: AuditReason::try_new("Should do nothing")?,
    };

    // Act
    f.bus()
        .execute::<AccountCommandCtx, IncreaseTrustScoreCommand, ()>(f.command_ctx().clone(), cmd)
        .await?;

    // Assert
    // L'opération s'exécute sans erreur, mais n'applique aucune modification d'état (No-Op transactionnel)
    f.account_assertions()
        .assert_account_state(f.account_id(), |acc| {
            assert_eq!(acc.governance().trust_score().value(), 100);
            assert_eq!(
                acc.version(),
                version_snapshot,
                "La version ne doit pas bouger"
            );
        })
        .await;

    f.account_assertions()
        .assert_no_events_for(f.account_id())
        .await;

    Ok(())
}
