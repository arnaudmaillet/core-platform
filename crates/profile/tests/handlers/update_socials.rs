use profile::commands::UpdateSocialsCommand;
use profile::context::ProfileCommandContext;
use profile::events::ProfileEvent;
use profile::types::{Handle, Socials};
use profile_test_utils::ProfileTestFixture;
use profile_test_utils::assertions::ProfileRepositoryAsserts;
use shared_kernel::command::CommandTarget;
use shared_kernel::core::{ErrorCode, Result, Versioned};
use shared_kernel::types::Url;
use uuid::Uuid;

#[tokio::test]
async fn test_update_socials_success() -> Result<()> {
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

    let socials = Socials::builder()
        .with_x(Url::try_new("https://x.com/alice")?)
        .with_website(Url::try_new("https://alice.dev")?)
        .build();

    let cmd = UpdateSocialsCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.profile_id(), version_snapshot),
        new_socials: Some(socials.clone()),
    };

    // Act
    f.bus()
        .execute::<ProfileCommandContext, UpdateSocialsCommand, ()>(f.command_ctx(), cmd)
        .await?;

    // Assert
    f.profile_repo()
        .assert_profile_state(f.profile_id(), |p| {
            assert_eq!(p.socials(), Some(&socials));
            assert_eq!(p.version(), version_snapshot + 1);
        })
        .await;

    f.profile_repo()
        .assert_captured_event_for(f.profile_id(), |event| match event {
            ProfileEvent::SocialsUpdated {
                profile_id,
                account_id,
                old_socials,
                new_socials: captured_new_socials,
                ..
            } => {
                assert_eq!(profile_id, &f.profile_id());
                assert_eq!(account_id, &f.account_id());
                assert_eq!(old_socials, &None);
                assert_eq!(captured_new_socials, &Some(socials));
            }
            _ => panic!("Type d'événement incorrect"),
        })
        .await;

    Ok(())
}

#[tokio::test]
async fn test_update_socials_technical_idempotency() -> Result<()> {
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

    let new_socials = Socials::builder()
        .with_x(Url::try_new("https://x.com/alice")?)
        .build();

    let cmd = UpdateSocialsCommand {
        command_id: cmd_id,
        target: CommandTarget::versioned(f.profile_id(), version_snapshot),
        new_socials: Some(new_socials),
    };

    // Act
    let result = f
        .bus()
        .execute::<ProfileCommandContext, UpdateSocialsCommand, ()>(f.command_ctx(), cmd)
        .await;

    // Assert
    assert!(result.is_ok());
    f.profile_repo().assert_no_events_for(f.profile_id()).await;

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
    f.given_slug_routing(
        f.profile_id(),
        &Handle::try_new("alice")?.to_sha256_hash(),
        f.region(),
    )
    .await;

    let cmd = UpdateSocialsCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.profile_id(), version_snapshot),
        new_socials: Some(socials),
    };

    // Act
    f.bus()
        .execute::<ProfileCommandContext, UpdateSocialsCommand, ()>(f.command_ctx(), cmd)
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
async fn test_update_socials_concurrency_conflict() -> Result<()> {
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

    let cmd = UpdateSocialsCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.profile_id(), 42),
        new_socials: None,
    };

    // Act
    let result = f
        .bus()
        .execute::<ProfileCommandContext, UpdateSocialsCommand, ()>(f.command_ctx(), cmd)
        .await;

    // Assert
    assert!(matches!(result, Err(e) if e.code == ErrorCode::ConcurrencyConflict));
    f.profile_repo().assert_no_events_for(f.profile_id()).await;

    Ok(())
}
