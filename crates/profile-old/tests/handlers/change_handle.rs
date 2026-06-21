use profile_old::commands::ChangeHandleCommand;
use profile_old::context::ProfileCommandCtx;
use profile_old::entities::Profile;
use profile_old::events::ProfileEvent;
use profile_old::types::Handle;
use profile_test_utils::ProfileTestFixture;
use profile_test_utils::assertions::ProfileRepositoryAsserts;
use shared_kernel::command::CommandTarget;
use shared_kernel::core::{ErrorCode, Result, Versioned};
use shared_kernel::types::{AccountId, ProfileId};
use uuid::Uuid;

#[tokio::test]
async fn test_change_handle_success() -> Result<()> {
    // Arrange
    let f = ProfileTestFixture::new();
    let old_handle = Handle::try_new("old.handle")?;

    let profile = f.builder("old.handle")?.build()?;
    let version_snapshot = profile.version();

    f.given_profile(profile).await;
    f.given_slug_routing(f.profile_id(), &old_handle.to_sha256_hash(), f.region())
        .await;

    let new_handle = Handle::try_new("new.handle")?;

    let cmd = ChangeHandleCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.profile_id(), version_snapshot),
        new_handle: new_handle.clone(),
    };

    // Act
    f.bus()
        .execute::<ProfileCommandCtx, ChangeHandleCommand, ()>(f.command_ctx().clone(), cmd)
        .await?;

    // Assert
    f.profile_repo()
        .assert_profile_state(f.profile_id(), |p| {
            assert_eq!(p.handle(), &new_handle);
            assert_eq!(p.version(), version_snapshot + 1);
        })
        .await;

    f.profile_repo()
        .assert_captured_event_for(f.profile_id(), |event| match event {
            ProfileEvent::HandleChanged {
                profile_id,
                account_id,
                old_handle: captured_old,
                new_handle: actual_new_handle,
                ..
            } => {
                assert_eq!(profile_id, &f.profile_id());
                assert_eq!(account_id, &f.account_id());
                assert_eq!(captured_old.as_str(), "old.handle");
                assert_eq!(actual_new_handle.as_str(), "new.handle");
            }
            _ => panic!(
                "Type d'événement incorrect. Attendu: HandleChanged, Reçu: {:?}",
                event
            ),
        })
        .await;

    Ok(())
}

#[tokio::test]
async fn test_change_handle_conflict_already_exists() -> Result<()> {
    // Arrange
    let f = ProfileTestFixture::new();
    let my_handle = Handle::try_new("my.handle")?;

    let profile = f.builder("my.handle")?.build()?;
    f.given_profile(profile).await;
    f.given_slug_routing(f.profile_id(), &my_handle.to_sha256_hash(), f.region())
        .await;

    let other_id = ProfileId::generate();
    let taken_handle = Handle::try_new("taken.handle")?;

    let other_profile =
        Profile::builder(AccountId::generate(), other_id, taken_handle.clone())?.build()?;

    f.given_profile(other_profile).await;
    f.given_slug_routing(other_id, &taken_handle.to_sha256_hash(), f.region())
        .await;

    let cmd = ChangeHandleCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.profile_id(), 0),
        new_handle: taken_handle,
    };

    // Act
    let result = f
        .bus()
        .execute::<ProfileCommandCtx, ChangeHandleCommand, ()>(f.command_ctx().clone(), cmd)
        .await;

    // Assert
    match result {
        Err(e) => {
            assert_eq!(e.code, ErrorCode::ConcurrencyConflict);
        }
        Ok(_) => panic!("Should have failed with ConcurrencyConflict because of LWT collision"),
    }

    f.profile_repo().assert_no_events_for(f.profile_id()).await;

    Ok(())
}

#[tokio::test]
async fn test_change_handle_business_idempotency() -> Result<()> {
    // Arrange
    let f = ProfileTestFixture::new();
    let handle = Handle::try_new("alice.handle")?;

    let profile = f.builder("alice.handle")?.build()?;
    let version_snapshot = profile.version();

    f.given_profile(profile).await;
    f.given_slug_routing(f.profile_id(), &handle.to_sha256_hash(), f.region())
        .await;

    let cmd = ChangeHandleCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.profile_id(), version_snapshot),
        new_handle: handle,
    };

    // Act
    f.bus()
        .execute::<ProfileCommandCtx, ChangeHandleCommand, ()>(f.command_ctx().clone(), cmd)
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
async fn test_change_handle_concurrency_conflict() -> Result<()> {
    // Arrange
    let f = ProfileTestFixture::new();
    let profile = f.builder("alice")?.build()?;
    f.given_profile(profile).await;

    let cmd = ChangeHandleCommand {
        command_id: Uuid::new_v4(),
        target: CommandTarget::versioned(f.profile_id(), 99),
        new_handle: Handle::try_new("new.alice")?,
    };

    // Act
    let result = f
        .bus()
        .execute::<ProfileCommandCtx, ChangeHandleCommand, ()>(f.command_ctx().clone(), cmd)
        .await;

    // Assert
    assert!(matches!(
        result,
        Err(e) if e.code == ErrorCode::ConcurrencyConflict
    ));

    f.profile_repo().assert_no_events_for(f.profile_id()).await;

    Ok(())
}
