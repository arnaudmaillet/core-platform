// crates/account/src/application/use_cases/moderation/decrease_trust_score/decrease_trust_score_use_case_test.rs

use account_old::commands::moderation::DecreaseTrustScoreCommand;
use account_old::context::AccountCommandCtx;
use account_old::events::AccountEvent;
use account_old::types::{AccountState, TrustAmount, TrustScore};
use account_test_utils::asserts::AccountRepositoryAsserts;

use account_test_utils::AccountTestFixture;
use shared_kernel::command::CommandTarget;
use shared_kernel::core::{Error, Result, Versioned};
use shared_kernel::idempotency::IdempotencyRepository;
use shared_kernel::types::AuditReason;
use uuid::Uuid;

#[tokio::test]
async fn test_decrease_trust_score_success() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();

    // 1. Compte actif (score 100 par défaut via le builder)
    let account = f.builder()?.with_state(AccountState::ACTIVE).build()?;
    let version_snapshot = account.version();
    f.account_repo().insert(account);

    let reason = AuditReason::try_new("Suspicious activity")?;
    let cmd = DecreaseTrustScoreCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.account_id(), version_snapshot),
        region: f.server_region(),
        amount: TrustAmount::try_from(30)?,
        reason: reason.clone(),
    };

    // Act
    f.bus()
        .execute::<AccountCommandCtx, DecreaseTrustScoreCommand, ()>(f.command_ctx().clone(), cmd)
        .await?;

    // Assert
    f.account_assertions()
        .assert_account_state(f.account_id(), |acc| {
            assert_eq!(acc.governance().trust_score().value(), 70);
            assert_eq!(acc.version(), version_snapshot + 1);
        })
        .await;

    f.assert_outbox(1, Some(AccountEvent::TRUST_SCORE_PENALIZED));

    Ok(())
}

#[tokio::test]
async fn test_decrease_trust_score_clamping_and_shadowban() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();

    // On initialise le compte juste au niveau du seuil critique (ex: 20)
    let account = f
        .builder()?
        .with_state(AccountState::ACTIVE)
        .with_trust_score(TrustScore::from_raw(TrustScore::CRITICAL_THRESHOLD))
        .build()?;

    let version_snapshot = account.version();
    f.account_repo().insert(account);

    let cmd = DecreaseTrustScoreCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.account_id(), version_snapshot),
        region: f.server_region(),
        amount: TrustAmount::try_from(50)?, // 20 - 50 -> Clamping automatique à 0
        reason: AuditReason::try_new("Heavy violation")?,
    };

    // Act
    f.bus()
        .execute::<AccountCommandCtx, DecreaseTrustScoreCommand, ()>(f.command_ctx().clone(), cmd)
        .await?;

    // Assert
    f.account_assertions()
        .assert_account_state(f.account_id(), |acc| {
            assert_eq!(acc.governance().trust_score().value(), 0);
            assert!(acc.governance().is_shadowbanned());
            assert_eq!(acc.version(), version_snapshot + 1);
        })
        .await;

    // Validation du déclenchement automatique du Shadowban réactionnel
    let events = f.outbox_events();
    f.assert_outbox(2, None);

    assert!(events.contains(&AccountEvent::TRUST_SCORE_PENALIZED.to_string()));
    assert!(events.contains(&AccountEvent::SHADOWBAN_UPDATED.to_string()));

    Ok(())
}

#[tokio::test]
async fn test_decrease_trust_score_technical_idempotency() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();
    let cmd_id = Uuid::new_v4();

    // On simule une commande déjà interceptée au premier rideau par l'infra
    f.idempotency_repo().save(None, &cmd_id).await?;

    let account = f.builder()?.with_state(AccountState::ACTIVE).build()?;
    let version_snapshot = account.version();
    f.account_repo().insert(account);

    let cmd = DecreaseTrustScoreCommand {
        command_id: cmd_id,
        target: CommandTarget::versioned(f.account_id(), version_snapshot),
        region: f.server_region(),
        amount: TrustAmount::try_from(10)?,
        reason: AuditReason::try_new("Duplicate")?,
    };

    // Act
    let result = f
        .bus()
        .execute::<AccountCommandCtx, DecreaseTrustScoreCommand, ()>(f.command_ctx().clone(), cmd)
        .await;

    // Assert
    assert!(
        result.is_ok(),
        "L'idempotence technique doit court-circuiter de façon transparente (Ok)"
    );

    // L'état reste intègre et inchangé
    f.account_assertions()
        .assert_account_state(f.account_id(), |acc| {
            assert_eq!(acc.version(), version_snapshot);
        })
        .await;

    f.account_assertions()
        .assert_no_events_for(f.account_id())
        .await;

    Ok(())
}

#[tokio::test]
async fn test_decrease_trust_score_business_idempotency_at_floor() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();

    // Le statut BANNED force déjà le score à 0 au niveau des invariants du domaine
    let account = f.builder()?.with_state(AccountState::BANNED).build()?;
    let version_snapshot = account.version();
    f.account_repo().insert(account);

    let cmd = DecreaseTrustScoreCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.account_id(), version_snapshot),
        region: f.server_region(),
        amount: TrustAmount::try_from(10)?,
        reason: AuditReason::try_new("Already at zero")?,
    };

    // Act
    f.bus()
        .execute::<AccountCommandCtx, DecreaseTrustScoreCommand, ()>(f.command_ctx().clone(), cmd)
        .await?;

    // Assert
    // Pas de modification d'état ni d'incrément de version (No-Op transactionnel)
    f.account_assertions()
        .assert_account_state(f.account_id(), |acc| {
            assert_eq!(acc.governance().trust_score().value(), 0);
            assert_eq!(
                acc.version(),
                version_snapshot,
                "Pas de mutation si déjà au plancher"
            );
        })
        .await;

    f.account_assertions()
        .assert_no_events_for(f.account_id())
        .await;

    Ok(())
}

#[tokio::test]
async fn test_trust_decrease_succeeds_after_retry() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();
    let account = f.builder()?.with_state(AccountState::ACTIVE).build()?;
    let version_snapshot = account.version();
    f.account_repo().insert(account);

    // Erreur transitoire OCC simulée
    f.account_repo().set_error_once(Error::concurrency_conflict(
        "Database Version Conflict / OCC Concurrency failure",
    ));

    let cmd = DecreaseTrustScoreCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.account_id(), version_snapshot),
        region: f.server_region(),
        amount: TrustAmount::try_from(1)?,
        reason: AuditReason::try_new("Test")?,
    };

    // Act : Le Bus intercepte le conflit et retente l'exécution du use-case
    let result = f
        .bus()
        .execute::<AccountCommandCtx, DecreaseTrustScoreCommand, ()>(f.command_ctx().clone(), cmd)
        .await;

    // Assert
    assert!(
        result.is_ok(),
        "Le middleware de retry aurait dû intercepter le conflit et réussir au second essai"
    );

    f.assert_outbox(1, Some(AccountEvent::TRUST_SCORE_PENALIZED));

    Ok(())
}
