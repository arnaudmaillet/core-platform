use profile::commands::UpdateBioCommand;
use profile::context::ProfileCommandContext;
use profile::events::ProfileEvent;
use profile::types::{Bio, Handle};
use profile_test_utils::ProfileTestFixture;
use profile_test_utils::assertions::ProfileRepositoryAsserts;
use shared_kernel::command::CommandTarget;
use shared_kernel::core::{ErrorCode, Result, Versioned};
use uuid::Uuid;

#[tokio::test]
async fn test_update_bio_success() -> Result<()> {
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

    let new_bio = Some(Bio::try_new("Software Engineer & Rustacean")?);

    let cmd = UpdateBioCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.profile_id(), version_snapshot),
        new_bio: new_bio.clone(),
    };

    // Act
    f.bus()
        .execute::<ProfileCommandContext, UpdateBioCommand, ()>(f.command_ctx(), cmd)
        .await?;

    // Assert
    f.profile_repo()
        .assert_profile_state(f.profile_id(), |p| {
            assert_eq!(p.bio(), new_bio.as_ref());
            assert_eq!(p.version(), version_snapshot + 1);
        })
        .await;

    f.profile_repo()
        .assert_captured_event_for(f.profile_id(), |event| match event {
            ProfileEvent::BioUpdated {
                profile_id,
                account_id,
                old_bio,
                new_bio: captured_new_bio,
                ..
            } => {
                assert_eq!(profile_id, &f.profile_id());
                assert_eq!(account_id, &f.account_id());
                assert_eq!(old_bio, &None);
                assert_eq!(captured_new_bio, &new_bio);
            }
            _ => panic!("Type d'événement incorrect"),
        })
        .await;

    Ok(())
}

#[tokio::test]
async fn test_update_bio_technical_idempotency() -> Result<()> {
    // Arrange
    let f = ProfileTestFixture::new();
    let cmd_id = Uuid::new_v4();
    f.idempotency_repo().seed(cmd_id);

    let profile = f.builder("alice")?.build()?;
    let initial_version = profile.version();
    f.given_profile(profile).await;
    f.given_slug_routing(
        f.profile_id(),
        &Handle::try_new("alice")?.to_sha256_hash(),
        f.region(),
    )
    .await;

    let cmd = UpdateBioCommand {
        command_id: cmd_id,
        target: CommandTarget::versioned(f.profile_id(), initial_version),
        new_bio: Some(Bio::try_new("Duplicate bio")?),
    };

    // Act
    let result = f
        .bus()
        .execute::<ProfileCommandContext, UpdateBioCommand, ()>(f.command_ctx(), cmd)
        .await;

    // Assert
    assert!(result.is_ok());
    f.profile_repo().assert_no_events_for(f.profile_id()).await;
    f.profile_repo()
        .assert_profile_state(f.profile_id(), |p| {
            assert_eq!(p.version(), initial_version);
        })
        .await;

    Ok(())
}

#[tokio::test]
async fn test_update_bio_business_idempotency() -> Result<()> {
    // Arrange
    let f = ProfileTestFixture::new();
    let bio = Bio::try_new("Already my bio")?;

    let profile = f.builder("alice")?.with_bio(bio.clone()).build()?;
    let version_snapshot = profile.version();
    f.given_profile(profile).await;
    f.given_slug_routing(
        f.profile_id(),
        &Handle::try_new("alice")?.to_sha256_hash(),
        f.region(),
    )
    .await;

    let cmd = UpdateBioCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.profile_id(), version_snapshot),
        new_bio: Some(bio),
    };

    // Act
    f.bus()
        .execute::<ProfileCommandContext, UpdateBioCommand, ()>(f.command_ctx(), cmd)
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
async fn test_update_bio_concurrency_conflict() -> Result<()> {
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

    let cmd = UpdateBioCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.profile_id(), 42),
        new_bio: Some(Bio::try_new("Conflict bio")?),
    };

    // Act
    let result = f
        .bus()
        .execute::<ProfileCommandContext, UpdateBioCommand, ()>(f.command_ctx(), cmd)
        .await;

    // Assert
    assert!(matches!(result, Err(e) if e.code == ErrorCode::ConcurrencyConflict));
    f.profile_repo().assert_no_events_for(f.profile_id()).await;

    Ok(())
}
