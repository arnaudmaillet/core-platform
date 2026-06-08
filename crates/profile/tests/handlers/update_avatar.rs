// crates/profile/src/application/commands/media/update_avatar/update_avatar_handler.rs
use profile::commands::UpdateAvatarCommand;
use profile::context::ProfileCommandContext;
use profile::events::ProfileEvent;
use profile_test_utils::ProfileTestFixture;
use shared_kernel::command::CommandTarget;
use shared_kernel::core::{ErrorCode, Result, Versioned};
use shared_kernel::types::Url;
use shared_kernel_test_utils::repositories::TransactionManagerStub;
use uuid::Uuid;

#[tokio::test]
async fn test_update_avatar_success() -> Result<()> {
    // Arrange
    let f = ProfileTestFixture::new();

    // Profil sans avatar au départ
    let profile = f.builder("alice")?.build()?;
    let version_snapshot = profile.version();
    f.given_profile(profile).await;

    let new_url = Url::try_new("https://cdn.test.com/new_avatar.png")?;

    let cmd = UpdateAvatarCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.profile_id(), version_snapshot),
        region: f.region(),
        new_avatar_url: new_url.clone(),
    };

    // Act
    f.bus()
        .execute::<ProfileCommandContext<TransactionManagerStub>, UpdateAvatarCommand, ()>(
            f.command_ctx().clone(),
            cmd,
        )
        .await?;

    // Assert
    let _ = f
        .assert_profile(|p| {
            assert_eq!(p.avatar(), Some(&new_url));
            assert_eq!(p.version(), version_snapshot + 1);
        })
        .await;

    f.assert_outbox(1, Some(ProfileEvent::AVATAR_UPDATED));

    Ok(())
}

#[tokio::test]
async fn test_update_avatar_technical_idempotency() -> Result<()> {
    // Arrange
    let f = ProfileTestFixture::new();
    let cmd_id = Uuid::new_v4();

    // 1. On "seed" le repo d'idempotence pour simuler que cette commande a déjà été traitée
    f.idempotency_repo().seed(cmd_id);

    let profile = f.builder("alice")?.build()?;
    f.given_profile(profile).await;

    let cmd = UpdateAvatarCommand {
        command_id: cmd_id, // Même ID que celui seedé
        target: CommandTarget::versioned(f.profile_id(), 0),
        region: f.region(),
        new_avatar_url: Url::try_new("https://cdn.test.com/new.png")?,
    };

    // Act
    let result = f
        .bus()
        .execute::<ProfileCommandContext<TransactionManagerStub>, UpdateAvatarCommand, ()>(
            f.command_ctx().clone(),
            cmd,
        )
        .await;

    // Assert
    // Doit retourner une erreur AlreadyExists (Command id)
    assert!(
        result.is_ok(),
        "L'idempotence technique doit être transparente (Ok)"
    );
    // On vérifie que rien n'a été émis
    f.assert_outbox(0, None);

    Ok(())
}

#[tokio::test]
async fn test_update_avatar_business_idempotency() -> Result<()> {
    // Arrange
    let f = ProfileTestFixture::new();
    let current_url = Url::try_new("https://cdn.test.com/existing.png")?;

    // Le profil a déjà cet avatar
    let profile = f
        .builder("alice")?
        .with_avatar(current_url.clone())
        .build()?;

    let version_snapshot = profile.version();
    f.given_profile(profile).await;

    let cmd = UpdateAvatarCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.profile_id(), version_snapshot),
        region: f.region(),
        new_avatar_url: current_url, // Même URL
    };

    // Act
    f.bus()
        .execute::<ProfileCommandContext<TransactionManagerStub>, UpdateAvatarCommand, ()>(
            f.command_ctx().clone(),
            cmd,
        )
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
async fn test_update_avatar_conflict() -> Result<()> {
    // Arrange
    let f = ProfileTestFixture::new();
    let profile = f.builder("alice")?.build()?;
    f.given_profile(profile).await;

    let cmd = UpdateAvatarCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.profile_id(), 10), // Mauvaise version
        region: f.region(),
        new_avatar_url: Url::try_new("https://cdn.test.com/fail.png")?,
    };

    // Act
    let result = f
        .bus()
        .execute::<ProfileCommandContext<TransactionManagerStub>, UpdateAvatarCommand, ()>(
            f.command_ctx().clone(),
            cmd,
        )
        .await;

    // Assert
    assert!(matches!(
        result,
        Err(e) if e.code == ErrorCode::ConcurrencyConflict
    ));

    Ok(())
}
