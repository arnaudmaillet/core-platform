use profile::commands::UpdateAvatarCommand;
use profile::context::ProfileCommandContext;
use profile::events::ProfileEvent;
use profile::types::Handle;
use profile_test_utils::ProfileTestFixture;
use profile_test_utils::assertions::ProfileRepositoryAsserts;
use shared_kernel::command::CommandTarget;
use shared_kernel::core::{ErrorCode, Result, Versioned};
use shared_kernel::types::Url;
use uuid::Uuid;

#[tokio::test]
async fn test_update_avatar_success() -> Result<()> {
    // Arrange
    let f = ProfileTestFixture::new();

    let profile = f.builder("alice")?.build()?;
    let version_snapshot = profile.version();
    f.given_profile(profile).await;
    // 💡 FIX : Index requis pour le validateur d'identité
    f.given_slug_routing(
        f.profile_id(),
        &Handle::try_new("alice")?.to_sha256_hash(),
        f.region(),
    )
    .await;

    let new_url = Url::try_new("https://cdn.test.com/new_avatar.png")?;
    let cmd = UpdateAvatarCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.profile_id(), version_snapshot),
        new_avatar_url: new_url.clone(),
    };

    // Act
    f.bus()
        .execute::<ProfileCommandContext, UpdateAvatarCommand, ()>(f.command_ctx(), cmd)
        .await?;

    // Assert
    f.profile_repo()
        .assert_profile_state(f.profile_id(), |p| {
            assert_eq!(p.avatar(), Some(&new_url));
            assert_eq!(p.version(), version_snapshot + 1);
        })
        .await;

    f.profile_repo()
        .assert_captured_event_for(f.profile_id(), |event| match event {
            ProfileEvent::AvatarUpdated {
                profile_id,
                account_id,
                old_avatar_url,
                new_avatar_url,
                ..
            } => {
                assert_eq!(profile_id, &f.profile_id());
                assert_eq!(account_id, &f.account_id());
                assert_eq!(old_avatar_url, &None);
                assert_eq!(new_avatar_url, &new_url);
            }
            _ => panic!("Type d'événement incorrect"),
        })
        .await;

    Ok(())
}

#[tokio::test]
async fn test_update_avatar_technical_idempotency() -> Result<()> {
    // Arrange
    let f = ProfileTestFixture::new();
    let cmd_id = Uuid::new_v4();
    f.idempotency_repo().seed(cmd_id);

    let profile = f.builder("alice")?.build()?;
    let version_snapshot = profile.version();
    f.given_profile(profile).await;
    f.given_slug_routing(
        f.profile_id(),
        &Handle::try_new("alice")?.to_sha256_hash(),
        f.region(),
    )
    .await;

    let cmd = UpdateAvatarCommand {
        command_id: cmd_id,
        target: CommandTarget::versioned(f.profile_id(), version_snapshot),
        new_avatar_url: Url::try_new("https://cdn.test.com/new.png")?,
    };

    // Act
    let result = f
        .bus()
        .execute::<ProfileCommandContext, UpdateAvatarCommand, ()>(f.command_ctx(), cmd)
        .await;

    // Assert
    assert!(result.is_ok());
    f.profile_repo().assert_no_events_for(f.profile_id()).await;

    Ok(())
}

#[tokio::test]
async fn test_update_avatar_business_idempotency() -> Result<()> {
    // Arrange
    let f = ProfileTestFixture::new();
    let current_url = Url::try_new("https://cdn.test.com/existing.png")?;

    let profile = f
        .builder("alice")?
        .with_avatar(current_url.clone())
        .build()?;
    let version_snapshot = profile.version();
    f.given_profile(profile).await;
    f.given_slug_routing(
        f.profile_id(),
        &Handle::try_new("alice")?.to_sha256_hash(),
        f.region(),
    )
    .await;

    let cmd = UpdateAvatarCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.profile_id(), version_snapshot),
        new_avatar_url: current_url,
    };

    // Act
    f.bus()
        .execute::<ProfileCommandContext, UpdateAvatarCommand, ()>(f.command_ctx(), cmd)
        .await?;

    // Assert
    f.profile_repo()
        .assert_profile_state(f.profile_id(), |p| {
            assert_eq!(p.version(), version_snapshot);
        })
        .await;

    f.profile_repo().assert_no_events_for(f.profile_id()).await;

    Ok(())
}

#[tokio::test]
async fn test_update_avatar_conflict() -> Result<()> {
    // Arrange
    let f = ProfileTestFixture::new();
    let profile = f.builder("alice")?.build()?;
    f.given_profile(profile).await;
    f.given_slug_routing(
        f.profile_id(),
        &Handle::try_new("alice")?.to_sha256_hash(),
        f.region(),
    )
    .await;

    let cmd = UpdateAvatarCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.profile_id(), 10),
        new_avatar_url: Url::try_new("https://cdn.test.com/fail.png")?,
    };

    // Act
    let result = f
        .bus()
        .execute::<ProfileCommandContext, UpdateAvatarCommand, ()>(f.command_ctx(), cmd)
        .await;

    // Assert
    assert!(matches!(result, Err(e) if e.code == ErrorCode::ConcurrencyConflict));
    f.profile_repo().assert_no_events_for(f.profile_id()).await;

    Ok(())
}
