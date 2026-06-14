use profile::commands::RemoveBannerCommand;
use profile::context::ProfileCommandCtx;
use profile::events::ProfileEvent;
use profile::types::Handle;
use profile_test_utils::ProfileTestFixture;
use profile_test_utils::assertions::ProfileRepositoryAsserts; // 💡 Import du trait d'assertions découplé
use shared_kernel::command::CommandTarget;
use shared_kernel::core::{ErrorCode, Result, Versioned};
use shared_kernel::types::Url;
use uuid::Uuid;

#[tokio::test]
async fn test_remove_banner_success() -> Result<()> {
    // Arrange
    let f = ProfileTestFixture::new();

    let banner_url = Url::try_new("https://cdn.test.com/banner.png")?;
    let profile = f
        .builder("alice")?
        .with_banner(banner_url.clone())
        .build()?;

    let version_snapshot = profile.version();
    f.given_profile(profile).await;
    f.given_slug_routing(
        f.profile_id(),
        &Handle::try_new("alice")?.to_sha256_hash(),
        f.region(),
    )
    .await;

    let cmd = RemoveBannerCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.profile_id(), version_snapshot),
    };

    // Act
    f.bus()
        .execute::<ProfileCommandCtx, RemoveBannerCommand, ()>(f.command_ctx().clone(), cmd)
        .await?;

    // Assert
    f.profile_repo()
        .assert_profile_state(f.profile_id(), |p| {
            assert!(p.banner().is_none());
            assert_eq!(p.version(), version_snapshot + 1);
        })
        .await;

    f.profile_repo()
        .assert_captured_event_for(f.profile_id(), |event| match event {
            ProfileEvent::BannerRemoved {
                profile_id,
                account_id,
                old_banner_url,
                ..
            } => {
                assert_eq!(profile_id, &f.profile_id());
                assert_eq!(account_id, &f.account_id());
                assert_eq!(old_banner_url.as_ref(), Some(&banner_url));
            }
            _ => panic!(
                "Type d'événement incorrect. Attendu: BannerRemoved, Reçu: {:?}",
                event
            ),
        })
        .await;

    Ok(())
}

#[tokio::test]
async fn test_remove_banner_business_idempotency() -> Result<()> {
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

    let cmd = RemoveBannerCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.profile_id(), version_snapshot),
    };

    // Act
    f.bus()
        .execute::<ProfileCommandCtx, RemoveBannerCommand, ()>(f.command_ctx().clone(), cmd)
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
async fn test_remove_banner_technical_idempotency() -> Result<()> {
    // Arrange
    let f = ProfileTestFixture::new();
    let cmd_id = Uuid::new_v4();

    f.idempotency_repo().seed(cmd_id);

    let profile = f
        .builder("alice")?
        .with_banner(Url::try_new("https://cdn.com/banner.png")?)
        .build()?;
    let version_snapshot = profile.version();

    f.given_profile(profile).await;
    f.given_slug_routing(
        f.profile_id(),
        &Handle::try_new("alice")?.to_sha256_hash(),
        f.region(),
    )
    .await;

    let cmd = RemoveBannerCommand {
        command_id: cmd_id,
        target: CommandTarget::versioned(f.profile_id(), version_snapshot),
    };

    // Act
    let result = f
        .bus()
        .execute::<ProfileCommandCtx, RemoveBannerCommand, ()>(f.command_ctx().clone(), cmd)
        .await;

    // Assert
    assert!(
        result.is_ok(),
        "L'idempotence technique doit être transparente (Ok)"
    );

    f.profile_repo()
        .assert_profile_state(f.profile_id(), |p| {
            assert!(p.banner().is_some());
            assert_eq!(p.version(), version_snapshot);
        })
        .await;

    f.profile_repo().assert_no_events_for(f.profile_id()).await;

    Ok(())
}

#[tokio::test]
async fn test_remove_banner_concurrency_conflict() -> Result<()> {
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

    let cmd = RemoveBannerCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.profile_id(), 123),
    };

    // Act
    let result = f
        .bus()
        .execute::<ProfileCommandCtx, RemoveBannerCommand, ()>(f.command_ctx().clone(), cmd)
        .await;

    // Assert
    assert!(matches!(
        result,
        Err(e) if e.code == ErrorCode::ConcurrencyConflict
    ));

    f.profile_repo().assert_no_events_for(f.profile_id()).await;

    Ok(())
}
