use profile_old::commands::UpdateDisplayNameCommand;
use profile_old::context::ProfileCommandCtx;
use profile_old::events::ProfileEvent;
use profile_old::types::{DisplayName, Handle};
use profile_test_utils::ProfileTestFixture;
use profile_test_utils::assertions::ProfileRepositoryAsserts;
use shared_kernel::command::CommandTarget;
use shared_kernel::core::{ErrorCode, Result, Versioned};
use uuid::Uuid;

#[tokio::test]
async fn test_update_display_name_success() -> Result<()> {
    // Arrange
    let f = ProfileTestFixture::new();
    let profile = f.builder("alice")?.build()?;
    let version_snapshot = profile.version();
    f.given_profile(profile).await;
    // 💡 Index requis pour le validateur d'identité
    f.given_slug_routing(
        f.profile_id(),
        &Handle::try_new("alice")?.to_sha256_hash(),
        f.region(),
    )
    .await;

    let new_name = DisplayName::try_new("new_name")?;

    let cmd = UpdateDisplayNameCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.profile_id(), version_snapshot),
        new_display_name: new_name.clone(),
    };

    // Act
    f.bus()
        .execute::<ProfileCommandCtx, UpdateDisplayNameCommand, ()>(f.command_ctx().clone(), cmd)
        .await?;

    // Assert
    f.profile_repo()
        .assert_profile_state(f.profile_id(), |p| {
            assert_eq!(p.display_name(), &new_name);
            assert_eq!(p.version(), version_snapshot + 1);
        })
        .await;

    f.profile_repo()
        .assert_captured_event_for(f.profile_id(), |event| match event {
            ProfileEvent::DisplayNameUpdated {
                profile_id,
                account_id,
                new_display_name,
                ..
            } => {
                assert_eq!(profile_id, &f.profile_id());
                assert_eq!(account_id, &f.account_id());
                assert_eq!(new_display_name, &new_name);
            }
            _ => panic!("Type d'événement incorrect"),
        })
        .await;

    Ok(())
}

#[tokio::test]
async fn test_update_display_name_technical_idempotency() -> Result<()> {
    // Arrange
    let f = ProfileTestFixture::new();
    let cmd_id = Uuid::new_v4();
    f.idempotency_repo().seed(cmd_id);

    let profile = f.builder("Original")?.build()?;
    let version_snapshot = profile.version();
    f.given_profile(profile).await;
    f.given_slug_routing(
        f.profile_id(),
        &Handle::try_new("Original")?.to_sha256_hash(),
        f.region(),
    )
    .await;

    let cmd = UpdateDisplayNameCommand {
        command_id: cmd_id,
        target: CommandTarget::versioned(f.profile_id(), version_snapshot),
        new_display_name: DisplayName::try_new("New Name")?,
    };

    // Act
    let result = f
        .bus()
        .execute::<ProfileCommandCtx, UpdateDisplayNameCommand, ()>(f.command_ctx().clone(), cmd)
        .await;

    // Assert
    assert!(result.is_ok());
    f.profile_repo().assert_no_events_for(f.profile_id()).await;

    Ok(())
}

#[tokio::test]
async fn test_update_display_name_business_idempotency() -> Result<()> {
    // Arrange
    let f = ProfileTestFixture::new();
    let name = DisplayName::try_new("Consistent Name")?;

    let profile = f
        .builder("alice")?
        .with_display_name(name.clone())
        .build()?;
    let version_snapshot = profile.version();
    f.given_profile(profile).await;
    f.given_slug_routing(
        f.profile_id(),
        &Handle::try_new("alice")?.to_sha256_hash(),
        f.region(),
    )
    .await;

    let cmd = UpdateDisplayNameCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.profile_id(), version_snapshot),
        new_display_name: name,
    };

    // Act
    f.bus()
        .execute::<ProfileCommandCtx, UpdateDisplayNameCommand, ()>(f.command_ctx().clone(), cmd)
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
async fn test_update_display_name_conflict() -> Result<()> {
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

    let cmd = UpdateDisplayNameCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.profile_id(), 5),
        new_display_name: DisplayName::try_new("wont_work")?,
    };

    // Act
    let result = f
        .bus()
        .execute::<ProfileCommandCtx, UpdateDisplayNameCommand, ()>(f.command_ctx().clone(), cmd)
        .await;

    // Assert
    assert!(matches!(result, Err(e) if e.code == ErrorCode::ConcurrencyConflict));
    f.profile_repo().assert_no_events_for(f.profile_id()).await;

    Ok(())
}
