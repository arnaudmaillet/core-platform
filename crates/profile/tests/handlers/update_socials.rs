// crates/profile/src/application/commands/metadata/update_social_links/update_social_links_handler.rs

use profile::commands::UpdateSocialsCommand;
use profile::context::ProfileCommandContext;
use profile::events::ProfileEvent;
use profile::types::Socials;
use profile_test_utils::ProfileTestFixture;
use shared_kernel::command::CommandTarget;
use shared_kernel::core::{ErrorCode, Result, Versioned};
use shared_kernel::types::Url;
use shared_kernel_test_utils::repositories::TransactionManagerStub;
use uuid::Uuid;

#[tokio::test]
async fn test_update_socials_success() -> Result<()> {
    // Arrange
    let f = ProfileTestFixture::new();
    let profile = f.builder("alice")?.build()?;
    let version_snapshot = profile.version();
    f.given_profile(profile).await;

    // On crée un set de liens sociaux
    let socials = Socials::builder()
        .with_x(Url::try_new("https://x.com/alice")?)
        .with_website(Url::try_new("https://alice.dev")?)
        .build();

    let cmd = UpdateSocialsCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.profile_id(), version_snapshot),
        region: f.region(),
        new_socials: Some(socials.clone()),
    };

    // Act
    f.bus()
        .execute::<ProfileCommandContext<TransactionManagerStub>, UpdateSocialsCommand, ()>(
            f.command_ctx().clone(),
            cmd,
        )
        .await?;

    // Assert
    let _ = f
        .assert_profile(|p| {
            assert_eq!(p.socials(), Some(&socials));
            assert_eq!(p.version(), version_snapshot + 1);
        })
        .await;

    f.assert_outbox(1, Some(ProfileEvent::SOCIALS_UPDATED));

    Ok(())
}

#[tokio::test]
async fn test_update_socials_technical_idempotency() -> Result<()> {
    // Arrange
    let f = ProfileTestFixture::new();
    let cmd_id = Uuid::new_v4();
    f.idempotency_repo().seed(cmd_id); // Simule que la commande a déjà été traitée

    let profile = f.builder("alice")?.build()?;
    f.given_profile(profile).await;

    // On crée un changement REEL pour forcer le handler à aller jusqu'au ctx.save()
    let new_socials = Socials::builder()
        .with_x(Url::try_new("https://x.com/alice")?)
        .build();

    let cmd = UpdateSocialsCommand {
        command_id: cmd_id,
        target: CommandTarget::versioned(f.profile_id(), 0),
        region: f.region(),
        new_socials: Some(new_socials), // Ici, c'est différent de l'état actuel (None)
    };

    // Act
    let result = f
        .bus()
        .execute::<ProfileCommandContext<TransactionManagerStub>, UpdateSocialsCommand, ()>(
            f.command_ctx().clone(),
            cmd,
        )
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
async fn test_update_socials_business_idempotency() -> Result<()> {
    // Arrange
    let f = ProfileTestFixture::new();
    let socials = Socials::builder()
        .with_github(Url::try_new("https://github.com/alice")?)
        .build();

    let profile = f.builder("alice")?.with_socials(socials.clone()).build()?;
    let version_snapshot = profile.version();
    f.given_profile(profile).await;

    let cmd = UpdateSocialsCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.profile_id(), version_snapshot),
        region: f.region(),
        new_socials: Some(socials), // Identique
    };

    // Act
    f.bus()
        .execute::<ProfileCommandContext<TransactionManagerStub>, UpdateSocialsCommand, ()>(
            f.command_ctx().clone(),
            cmd,
        )
        .await?;

    // Assert
    let _ = f
        .assert_profile(|p| {
            assert_eq!(p.version(), version_snapshot);
        })
        .await;
    f.assert_outbox(0, None);

    Ok(())
}

#[tokio::test]
async fn test_update_socials_concurrency_conflict() -> Result<()> {
    // Arrange
    let f = ProfileTestFixture::new();
    let profile = f.builder("alice")?.build()?;
    f.given_profile(profile).await;

    let cmd = UpdateSocialsCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.profile_id(), 42), // Version erronée
        region: f.region(),
        new_socials: None,
    };

    // Act
    let result = f
        .bus()
        .execute::<ProfileCommandContext<TransactionManagerStub>, UpdateSocialsCommand, ()>(
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
