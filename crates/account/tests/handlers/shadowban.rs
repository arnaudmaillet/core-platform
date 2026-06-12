// crates/account/src/application/use_cases/moderation/shadowban/shadowban_use_case_test.rs

use account::commands::moderation::ShadowbanCommand;
use account::context::AccountCommandCtx;
use account::entities::AccountGovernanceBuilder;
use account::events::AccountEvent;
use account::types::AccountState;
use account_test_utils::asserts::AccountRepositoryAsserts;

use account_test_utils::AccountTestFixture;
use shared_kernel::command::CommandTarget;
use shared_kernel::core::{Result, Versioned};
use shared_kernel::idempotency::IdempotencyRepository;
use shared_kernel::types::AuditReason;
use uuid::Uuid;

#[tokio::test]
async fn test_shadowban_account_success() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();

    // 1. Compte sain et actif
    let account = f.builder()?.with_state(AccountState::ACTIVE).build()?;
    let version_snapshot = account.version();
    f.account_repo().insert(account);

    let reason = AuditReason::try_new("Spam behavior detected")?;
    let cmd = ShadowbanCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.account_id(), version_snapshot),
        region: f.region(),
        reason: reason.clone(),
    };

    // Act
    f.bus()
        .execute::<AccountCommandCtx, ShadowbanCommand, ()>(f.command_ctx().clone(), cmd)
        .await?;

    // Assert
    f.account_assertions()
        .assert_account_state(f.account_id(), |acc| {
            assert!(acc.governance().is_shadowbanned());
            assert_eq!(acc.version(), version_snapshot + 1);
        })
        .await;

    f.assert_outbox(1, Some(AccountEvent::SHADOWBAN_UPDATED));

    Ok(())
}

#[tokio::test]
async fn test_shadowban_technical_idempotency() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();
    let cmd_id = Uuid::new_v4();

    // On simule une commande déjà traitée techniquement interceptée au premier rideau
    f.idempotency_repo().save(None, &cmd_id).await?;

    let account = f.builder()?.with_state(AccountState::ACTIVE).build()?;
    let version_snapshot = account.version();
    f.account_repo().insert(account);

    let cmd = ShadowbanCommand {
        command_id: cmd_id,
        target: CommandTarget::versioned(f.account_id(), version_snapshot),
        region: f.region(),
        reason: AuditReason::try_new("Duplicate network call")?,
    };

    // Act
    let result = f
        .bus()
        .execute::<AccountCommandCtx, ShadowbanCommand, ()>(f.command_ctx().clone(), cmd)
        .await;

    // Assert
    assert!(
        result.is_ok(),
        "L'idempotence technique doit court-circuiter de façon transparente (Ok)"
    );

    // L'agrégat en base n'a subi aucune mutation
    f.account_assertions()
        .assert_account_state(f.account_id(), |acc| {
            assert!(!acc.governance().is_shadowbanned());
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
async fn test_shadowban_business_idempotency() -> Result<()> {
    // Arrange
    let f = AccountTestFixture::new();

    // Idempotence métier : Le compte est déjà configuré comme shadowbanné via le builder
    let account = f
        .builder()?
        .with_state(AccountState::ACTIVE)
        .governance(|g: AccountGovernanceBuilder| g.with_shadowban(true))
        .build()?;

    let version_snapshot = account.version();
    f.account_repo().insert(account);

    let cmd = ShadowbanCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.account_id(), version_snapshot),
        region: f.region(),
        reason: AuditReason::try_new("Second report")?,
    };

    // Act
    f.bus()
        .execute::<AccountCommandCtx, ShadowbanCommand, ()>(f.command_ctx().clone(), cmd)
        .await?;

    // Assert
    // L'opération s'exécute avec succès mais ne produit aucune mutation (No-Op transactionnel)
    f.account_assertions()
        .assert_account_state(f.account_id(), |acc| {
            assert!(acc.governance().is_shadowbanned());
            assert_eq!(
                acc.version(),
                version_snapshot,
                "La version ne doit pas bouger si l'état était déjà identique"
            );
        })
        .await;

    // Aucun événement métier produit puisque l'état est inchangé
    f.account_assertions()
        .assert_no_events_for(f.account_id())
        .await;

    Ok(())
}
