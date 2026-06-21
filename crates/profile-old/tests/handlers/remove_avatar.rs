use profile_old::commands::RemoveAvatarCommand;
use profile_old::context::ProfileCommandCtx;
use profile_old::events::ProfileEvent;
use profile_old::types::Handle;
use profile_test_utils::ProfileTestFixture;
use profile_test_utils::assertions::ProfileRepositoryAsserts;
use shared_kernel::command::CommandTarget;
use shared_kernel::core::{ErrorCode, Result, Versioned};
use shared_kernel::types::Url;
use uuid::Uuid;

#[tokio::test]
async fn test_remove_avatar_success() -> Result<()> {
    // Arrange
    let f = ProfileTestFixture::new();

    let avatar_url = Url::try_new("https://cdn.test.com/avatar.png")?;
    let profile = f
        .builder("alice")?
        .with_avatar(avatar_url.clone())
        .build()?;

    let version_snapshot = profile.version();

    f.given_profile(profile).await;
    f.given_slug_routing(
        f.profile_id(),
        &Handle::try_new("alice")?.to_sha256_hash(),
        f.region(),
    )
    .await;

    let cmd = RemoveAvatarCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.profile_id(), version_snapshot),
    };

    // Act
    f.bus()
        .execute::<ProfileCommandCtx, RemoveAvatarCommand, ()>(f.command_ctx().clone(), cmd)
        .await?;

    // Assert
    f.profile_repo()
        .assert_profile_state(f.profile_id(), |p| {
            assert!(p.avatar().is_none());
            assert_eq!(p.version(), version_snapshot + 1);
        })
        .await;

    f.profile_repo()
        .assert_captured_event_for(f.profile_id(), |event| match event {
            ProfileEvent::AvatarRemoved {
                profile_id,
                account_id,
                old_avatar_url,
                ..
            } => {
                assert_eq!(profile_id, &f.profile_id());
                assert_eq!(account_id, &f.account_id());
                assert_eq!(old_avatar_url.as_ref(), Some(&avatar_url));
            }
            _ => panic!(
                "Type d'événement incorrect. Attendu: AvatarRemoved, Reçu: {:?}",
                event
            ),
        })
        .await;

    Ok(())
}

#[tokio::test]
async fn test_remove_avatar_business_idempotency() -> Result<()> {
    // Arrange
    let f = ProfileTestFixture::new();

    let profile = f.builder("alice")?.build()?;
    let version_snapshot = profile.version();

    f.given_profile(profile).await;
    f.given_slug_routing(
        f.profile_id(),
        &Handle::try_new("alice")?.to_sha256_hash(),
        f.region(),
    )
    .await;

    let cmd = RemoveAvatarCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.profile_id(), version_snapshot),
    };

    // Act
    f.bus()
        .execute::<ProfileCommandCtx, RemoveAvatarCommand, ()>(f.command_ctx().clone(), cmd)
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
async fn test_remove_avatar_technical_idempotency() -> Result<()> {
    // Arrange
    let f = ProfileTestFixture::new();
    let cmd_id = Uuid::new_v4();

    f.idempotency_repo().seed(cmd_id);

    let profile = f
        .builder("alice")?
        .with_avatar(Url::try_new("https://cdn.com/avatar.png")?)
        .build()?;
    let version_snapshot = profile.version();

    f.given_profile(profile).await;
    f.given_slug_routing(
        f.profile_id(),
        &Handle::try_new("alice")?.to_sha256_hash(),
        f.region(),
    )
    .await;

    let cmd = RemoveAvatarCommand {
        command_id: cmd_id,
        target: CommandTarget::versioned(f.profile_id(), version_snapshot),
    };

    // Act
    let result = f
        .bus()
        .execute::<ProfileCommandCtx, RemoveAvatarCommand, ()>(f.command_ctx().clone(), cmd)
        .await;

    // Assert
    assert!(
        result.is_ok(),
        "L'idempotence technique doit être transparente (Ok)"
    );

    f.profile_repo()
        .assert_profile_state(f.profile_id(), |p| {
            assert!(p.avatar().is_some());
            assert_eq!(p.version(), version_snapshot);
        })
        .await;

    f.profile_repo().assert_no_events_for(f.profile_id()).await;

    Ok(())
}

#[tokio::test]
async fn test_remove_avatar_concurrency_conflict() -> Result<()> {
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

    let cmd = RemoveAvatarCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.profile_id(), 42),
    };

    // Act
    let result = f
        .bus()
        .execute::<ProfileCommandCtx, RemoveAvatarCommand, ()>(f.command_ctx().clone(), cmd)
        .await;

    // Assert
    assert!(matches!(
        result,
        Err(e) if e.code == ErrorCode::ConcurrencyConflict
    ));

    f.profile_repo().assert_no_events_for(f.profile_id()).await;

    Ok(())
}
