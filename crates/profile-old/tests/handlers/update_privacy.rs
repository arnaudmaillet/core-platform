use profile_old::commands::UpdatePrivacyCommand;
use profile_old::context::ProfileCommandCtx;
use profile_old::events::ProfileEvent;
use profile_old::types::Handle;
use profile_test_utils::ProfileTestFixture;
use profile_test_utils::assertions::ProfileRepositoryAsserts;
use shared_kernel::command::CommandTarget;
use shared_kernel::core::{ErrorCode, Result, Versioned};
use uuid::Uuid;

#[tokio::test]
async fn test_update_privacy_success() -> Result<()> {
    // Arrange
    let f = ProfileTestFixture::new();

    // On part d'un profil public
    let profile = f.builder("alice")?.with_privacy(false).build()?;
    let version_snapshot = profile.version();
    f.given_profile(profile).await;
    f.given_slug_routing(
        f.profile_id(),
        &Handle::try_new("alice")?.to_sha256_hash(),
        f.region(),
    )
    .await;

    let cmd = UpdatePrivacyCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.profile_id(), version_snapshot),
        is_private: true,
    };

    // Act
    f.bus()
        .execute::<ProfileCommandCtx, UpdatePrivacyCommand, ()>(f.command_ctx().clone(), cmd)
        .await?;

    // Assert
    f.profile_repo()
        .assert_profile_state(f.profile_id(), |p| {
            assert!(p.is_private());
            assert_eq!(p.version(), version_snapshot + 1);
        })
        .await;

    f.profile_repo()
        .assert_captured_event_for(f.profile_id(), |event| match event {
            ProfileEvent::PrivacyChanged {
                profile_id,
                account_id,
                is_private,
                ..
            } => {
                assert_eq!(profile_id, &f.profile_id());
                assert_eq!(account_id, &f.account_id());
                assert!(*is_private);
            }
            _ => panic!("Type d'événement incorrect"),
        })
        .await;

    Ok(())
}

#[tokio::test]
async fn test_update_privacy_business_idempotency() -> Result<()> {
    // Arrange
    let f = ProfileTestFixture::new();

    // Le profil est déjà en privé
    let profile = f.builder("alice")?.with_privacy(true).build()?;
    let version_snapshot = profile.version();
    f.given_profile(profile).await;
    f.given_slug_routing(
        f.profile_id(),
        &Handle::try_new("alice")?.to_sha256_hash(),
        f.region(),
    )
    .await;

    let cmd = UpdatePrivacyCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.profile_id(), version_snapshot),
        is_private: true,
    };

    // Act
    f.bus()
        .execute::<ProfileCommandCtx, UpdatePrivacyCommand, ()>(f.command_ctx().clone(), cmd)
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
async fn test_update_privacy_concurrency_conflict() -> Result<()> {
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

    let cmd = UpdatePrivacyCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.profile_id(), 99),
        is_private: true,
    };

    // Act
    let result = f
        .bus()
        .execute::<ProfileCommandCtx, UpdatePrivacyCommand, ()>(f.command_ctx().clone(), cmd)
        .await;

    // Assert
    assert!(matches!(result, Err(e) if e.code == ErrorCode::ConcurrencyConflict));
    f.profile_repo().assert_no_events_for(f.profile_id()).await;

    Ok(())
}
