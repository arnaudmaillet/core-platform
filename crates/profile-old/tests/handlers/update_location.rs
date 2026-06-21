use profile_old::commands::UpdateLocationCommand;
use profile_old::context::ProfileCommandCtx;
use profile_old::events::ProfileEvent;
use profile_old::types::{Handle, Location};
use profile_test_utils::ProfileTestFixture;
use profile_test_utils::assertions::ProfileRepositoryAsserts;
use shared_kernel::command::CommandTarget;
use shared_kernel::core::{ErrorCode, Result, Versioned};
use uuid::Uuid;

#[tokio::test]
async fn test_update_location_success() -> Result<()> {
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

    let new_location = Some(Location::try_new("Paris, France")?);

    let cmd = UpdateLocationCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.profile_id(), version_snapshot),
        new_location: new_location.clone(),
    };

    // Act
    f.bus()
        .execute::<ProfileCommandCtx, UpdateLocationCommand, ()>(f.command_ctx().clone(), cmd)
        .await?;

    // Assert
    f.profile_repo()
        .assert_profile_state(f.profile_id(), |p| {
            assert_eq!(p.location(), new_location.as_ref());
            assert_eq!(p.version(), version_snapshot + 1);
        })
        .await;

    f.profile_repo()
        .assert_captured_event_for(f.profile_id(), |event| match event {
            ProfileEvent::LocationUpdated {
                profile_id,
                account_id,
                old_location,
                new_location: captured_new_location,
                ..
            } => {
                assert_eq!(profile_id, &f.profile_id());
                assert_eq!(account_id, &f.account_id());
                assert_eq!(old_location, &None);
                assert_eq!(captured_new_location, &new_location);
            }
            _ => panic!("Type d'événement incorrect"),
        })
        .await;

    Ok(())
}

#[tokio::test]
async fn test_update_location_technical_idempotency() -> Result<()> {
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

    let cmd = UpdateLocationCommand {
        command_id: cmd_id,
        target: CommandTarget::versioned(f.profile_id(), version_snapshot),
        new_location: Some(Location::try_new("Tokyo, Japan")?),
    };

    // Act
    let result = f
        .bus()
        .execute::<ProfileCommandCtx, UpdateLocationCommand, ()>(f.command_ctx().clone(), cmd)
        .await;

    // Assert
    assert!(result.is_ok());
    f.profile_repo().assert_no_events_for(f.profile_id()).await;

    Ok(())
}

#[tokio::test]
async fn test_update_location_business_idempotency() -> Result<()> {
    // Arrange
    let f = ProfileTestFixture::new();
    let location = Location::try_new("Montreal, Canada")?;

    let profile = f
        .builder("alice")?
        .with_location(location.clone())
        .build()?;
    let version_snapshot = profile.version();
    f.given_profile(profile).await;
    f.given_slug_routing(
        f.profile_id(),
        &Handle::try_new("alice")?.to_sha256_hash(),
        f.region(),
    )
    .await;

    let cmd = UpdateLocationCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.profile_id(), version_snapshot),
        new_location: Some(location),
    };

    // Act
    f.bus()
        .execute::<ProfileCommandCtx, UpdateLocationCommand, ()>(f.command_ctx().clone(), cmd)
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
async fn test_update_location_concurrency_conflict() -> Result<()> {
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

    let cmd = UpdateLocationCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.profile_id(), 123),
        new_location: Some(Location::try_new("Nowhere")?),
    };

    // Act
    let result = f
        .bus()
        .execute::<ProfileCommandCtx, UpdateLocationCommand, ()>(f.command_ctx().clone(), cmd)
        .await;

    // Assert
    assert!(matches!(result, Err(e) if e.code == ErrorCode::ConcurrencyConflict));
    f.profile_repo().assert_no_events_for(f.profile_id()).await;

    Ok(())
}
