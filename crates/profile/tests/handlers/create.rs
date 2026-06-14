use profile::commands::CreateProfileCommand;
use profile::context::ProfileCommandCtx;
use profile::entities::Profile;
use profile::events::ProfileEvent;
use profile::types::Handle;
use profile_test_utils::ProfileTestFixture;
use profile_test_utils::assertions::ProfileRepositoryAsserts;
use shared_kernel::command::CommandTarget;
use shared_kernel::core::{ErrorCode, Result, Versioned};
use shared_kernel::types::ProfileId;
use uuid::Uuid;

#[tokio::test]
async fn test_create_profile_success() -> Result<()> {
    // Arrange
    let f = ProfileTestFixture::new();
    let generated_profile_id = ProfileId::generate();
    let handle = Handle::try_new("bob_dev")?;
    let target = CommandTarget::stateless(generated_profile_id);

    let cmd = CreateProfileCommand {
        command_id: Uuid::new_v4(),
        target,
        region: f.region(),
        account_id: f.account_id(),
        handle: handle.clone(),
    };

    // Act
    f.bus()
        .execute::<ProfileCommandCtx, CreateProfileCommand, ()>(f.creation_ctx(f.region()), cmd)
        .await?;

    // Assert
    f.profile_repo()
        .assert_profile_state(generated_profile_id, |p| {
            assert_eq!(p.profile_id(), generated_profile_id);
            assert_eq!(p.handle().as_str(), "bob_dev");
            assert_eq!(p.version(), 1);
        })
        .await;

    f.profile_repo()
        .assert_captured_event_for(generated_profile_id, |event| match event {
            ProfileEvent::ProfileCreated {
                profile_id,
                account_id,
                handle: captured_handle,
                ..
            } => {
                assert_eq!(profile_id, &generated_profile_id);
                assert_eq!(account_id, &f.account_id());
                assert_eq!(captured_handle, &handle);
            }
            _ => panic!(
                "Type d'événement incorrect. Attendu: ProfileCreated, Reçu: {:?}",
                event
            ),
        })
        .await;

    Ok(())
}

#[tokio::test]
async fn test_create_profile_technical_idempotency() -> Result<()> {
    // Arrange
    let f = ProfileTestFixture::new();
    let cmd_id = Uuid::new_v4();
    let profile_id = ProfileId::generate();

    f.idempotency_repo().seed(cmd_id);

    let existing_profile = f.builder("bob_dev")?.with_profile_id(profile_id).build()?;
    f.given_profile(existing_profile).await;

    let target = CommandTarget::stateless(profile_id);

    let cmd = CreateProfileCommand {
        command_id: cmd_id,
        target,
        region: f.region(),
        account_id: f.account_id(),
        handle: Handle::try_new("bob_dev")?,
    };

    // Act
    let result = f
        .bus()
        .execute::<ProfileCommandCtx, CreateProfileCommand, ()>(f.creation_ctx(f.region()), cmd)
        .await;

    // Assert
    assert!(
        result.is_ok(),
        "Le retry technique d'une création doit être transparent et renvoyer Ok(())"
    );

    f.profile_repo().assert_no_events_for(profile_id).await;

    Ok(())
}

#[tokio::test]
async fn test_create_profile_conflict_handle() -> Result<()> {
    // Arrange
    let f = ProfileTestFixture::new();
    let duplicated_handle = "taken_handle";
    let other_profile_id = ProfileId::generate();

    let handle_vo = Handle::try_new(duplicated_handle)?;
    let other_profile = Profile::builder(
        shared_kernel::types::AccountId::generate(),
        other_profile_id,
        handle_vo.clone(),
    )?
    .build()?;

    f.given_profile(other_profile).await;
    f.given_slug_routing(other_profile_id, &handle_vo.to_sha256_hash(), f.region())
        .await;

    let target_id = ProfileId::generate();
    let target = CommandTarget::stateless(target_id);

    let cmd = CreateProfileCommand {
        command_id: Uuid::new_v4(),
        target,
        region: f.region(),
        account_id: f.account_id(),
        handle: handle_vo,
    };

    // Act
    let result = f
        .bus()
        .execute::<ProfileCommandCtx, CreateProfileCommand, ()>(f.creation_ctx(f.region()), cmd)
        .await;

    // Assert
    assert!(
        matches!(&result, Err(e) if e.code == ErrorCode::AlreadyExists),
        "Tenter d'utiliser un Handle déjà pris doit lever un AlreadyExists. Reçu: {:?}",
        result
    );

    f.profile_repo().assert_no_events_for(target_id).await;

    Ok(())
}
