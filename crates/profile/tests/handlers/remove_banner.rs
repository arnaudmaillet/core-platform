// crates/profile/src/application/commands/media/remove_banner/remove_banner_handler.rs

use profile::commands::RemoveBannerCommand;
use profile::context::ProfileCommandContext;
use profile::events::ProfileEvent;
use profile_test_utils::ProfileTestFixture;
use shared_kernel::command::CommandTarget;
use shared_kernel::core::{ErrorCode, Result, Versioned};
use shared_kernel::types::Url;
use shared_kernel_test_utils::repositories::TransactionManagerStub;
use uuid::Uuid;

#[tokio::test]
async fn test_remove_banner_success() -> Result<()> {
    // Arrange
    let f = ProfileTestFixture::new();

    // On crée un profil avec une bannière
    let banner_url = Url::try_new("https://cdn.test.com/banner.png")?;
    let profile = f.builder("alice")?.with_banner(banner_url).build()?;

    let version_snapshot = profile.version();
    f.given_profile(profile).await;

    let cmd = RemoveBannerCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::new(f.profile_id(), f.region(), version_snapshot),
    };

    // Act
    f.bus()
        .execute::<ProfileCommandContext<TransactionManagerStub>, RemoveBannerCommand, ()>(f.command_ctx().clone(), cmd)
        .await?;

    // Assert
    let _ = f
        .assert_profile(|p| {
            assert!(p.banner().is_none());
            assert_eq!(p.version(), version_snapshot + 1);
        })
        .await;

    // Vérification de l'événement spécifique à la bannière
    f.assert_outbox(1, Some(ProfileEvent::BANNER_REMOVED));

    Ok(())
}

#[tokio::test]
async fn test_remove_banner_technical_idempotency() -> Result<()> {
    // Arrange
    let f = ProfileTestFixture::new();
    let cmd_id = Uuid::new_v4();

    // 1. On seed l'idempotence pour simuler un traitement déjà effectué
    f.idempotency_repo().seed(cmd_id);

    // 2. On crée un profil avec une bannière
    let profile = f
        .builder("alice")?
        .with_banner(Url::try_new("https://cdn.com/banner.png")?)
        .build()?;
    f.given_profile(profile).await;

    let cmd = RemoveBannerCommand {
        command_id: cmd_id,
        target: CommandTarget::new(f.profile_id(), f.region(), 0),
    };

    // Act
    let result = f
        .bus()
        .execute::<ProfileCommandContext<TransactionManagerStub>, RemoveBannerCommand, ()>(f.command_ctx().clone(), cmd)
        .await;

    // Assert
    // On s'attend à l'erreur technique d'idempotence
    assert!(
        result.is_ok(),
        "L'idempotence technique doit être transparente (Ok)"
    );

    // On vérifie que la bannière est toujours présente en base (car le save a été bloqué)
    let _ = f
        .assert_profile(|p| {
            let _ = assert!(p.banner().is_some());
        })
        .await;

    // Pas d'événement émis
    f.assert_outbox(0, None);

    Ok(())
}

#[tokio::test]
async fn test_remove_banner_business_idempotency() -> Result<()> {
    // Arrange
    let f = ProfileTestFixture::new();

    // Le profil n'a déjà pas de bannière
    let profile = f.builder("alice")?.build()?;

    let version_snapshot = profile.version();
    f.given_profile(profile).await;

    let cmd = RemoveBannerCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::new(f.profile_id(), f.region(), version_snapshot),
    };

    // Act
    f.bus()
        .execute::<ProfileCommandContext<TransactionManagerStub>, RemoveBannerCommand, ()>(f.command_ctx().clone(), cmd)
        .await?;

    // Assert
    let _ = f
        .assert_profile(|p| {
            assert_eq!(p.version(), version_snapshot); // Pas de changement de version
        })
        .await;

    f.assert_outbox(0, None);

    Ok(())
}

#[tokio::test]
async fn test_remove_banner_concurrency_conflict() -> Result<()> {
    // Arrange
    let f = ProfileTestFixture::new();
    let profile = f.builder("alice")?.build()?;
    f.given_profile(profile).await;

    let cmd = RemoveBannerCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::new(f.profile_id(), f.region(), 123), // Version désuète
    };

    // Act
    let result = f
        .bus()
        .execute::<ProfileCommandContext<TransactionManagerStub>, RemoveBannerCommand, ()>(f.command_ctx().clone(), cmd)
        .await;

    // Assert
    assert!(matches!(
        result,
        Err(e) if e.code == ErrorCode::ConcurrencyConflict
    ));

    Ok(())
}
