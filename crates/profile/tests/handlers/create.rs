use profile::commands::CreateProfileCommand;
use profile::context::ProfileCommandContext;
use profile::events::ProfileEvent;
use profile::repositories::ProfileRepository;
use profile::types::Handle;
use profile_test_utils::ProfileTestFixture;
use shared_kernel::core::{ErrorCode, Result, Versioned};
use shared_kernel::types::ProfileId;
use shared_kernel_test_utils::repositories::TransactionManagerStub;
use uuid::Uuid;

#[tokio::test]
async fn test_create_profile_success() -> Result<()> {
    let f = ProfileTestFixture::new();
    let generated_profile_id = ProfileId::generate();
    let creation_ctx = f.app_ctx().creation_command(f.region());

    let cmd = CreateProfileCommand {
        command_id: Uuid::new_v4(),
        profile_id: generated_profile_id,
        account_id: f.account_id(),
        handle: Handle::try_new("bob_dev")?,
        region: f.region(),
    };

    // Act - On exécute avec le ProfileCommandContext
    f.bus()
        .execute::<ProfileCommandContext<TransactionManagerStub>, CreateProfileCommand, ()>(
            creation_ctx,
            cmd.clone(),
        )
        .await?;

    // Assert
    let saved_profile = f
        .profile_repo()
        .find_by_handle(&cmd.handle, f.region(), None)
        .await?
        .expect("Le profil aurait dû être enregistré en base");

    assert_eq!(saved_profile.profile_id(), generated_profile_id);
    assert_eq!(saved_profile.handle().as_str(), "bob_dev");
    assert_eq!(saved_profile.version(), 1);

    f.assert_outbox(1, Some(ProfileEvent::PROFILE_CREATED));

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
    f.profile_repo()
        .save_direct(f.region(), existing_profile)
        .await;

    let cmd = CreateProfileCommand {
        command_id: cmd_id,
        profile_id,
        account_id: f.account_id(),
        handle: Handle::try_new("bob_dev")?,
        region: f.region(),
    };

    let result = f
        .bus()
        .execute::<ProfileCommandContext<TransactionManagerStub>, CreateProfileCommand, ()>(
            f.command_ctx().clone(),
            cmd,
        )
        .await;

    // Assert
    assert!(
        result.is_ok(),
        "Le retry technique d'une création doit être transparent et renvoyer Ok(())"
    );

    f.assert_outbox(0, None);

    Ok(())
}

#[tokio::test]
async fn test_create_profile_conflict_handle() -> Result<()> {
    // Arrange
    let f = ProfileTestFixture::new();
    let duplicated_handle = "taken_handle";
    let other_profile_id = ProfileId::generate();
    let f_other = f.clone_with_profile_id(other_profile_id);

    let profile_with_handle = f_other.builder(duplicated_handle)?.build()?;
    f_other
        .profile_repo()
        .save_direct(f.region(), profile_with_handle)
        .await;

    // 2. On tente de créer un NOUVEAU profil avec le même handle usurpé
    let cmd = CreateProfileCommand {
        command_id: Uuid::new_v4(),
        profile_id: ProfileId::generate(),
        account_id: f.account_id(),
        handle: Handle::try_new(duplicated_handle)?,
        region: f.region(),
    };

    // Act
    let result = f
        .bus()
        .execute::<ProfileCommandContext<TransactionManagerStub>, CreateProfileCommand, ()>(
            f.command_ctx().clone(),
            cmd,
        )
        .await;

    // Assert
    assert!(
        matches!(result, Err(e) if e.code == ErrorCode::AlreadyExists),
        "Tenter d'utiliser un Handle déjà pris dans la même région doit lever un AlreadyExists"
    );

    Ok(())
}
