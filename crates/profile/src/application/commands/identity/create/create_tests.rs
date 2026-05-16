// crates/profile/src/application/use_cases/identity/create_profile/mod.rs (ou fichier de test dédié)

#[cfg(test)]
mod tests {
    use crate::context::ProfileContext;
    use crate::events::ProfileEvent;
    use crate::repositories::ProfileRepository;
    use crate::types::{Handle, ProfileId};
    use crate::{application::utils::ProfileTestFixture, commands::CreateProfileCommand};
    use shared_kernel::core::{ErrorCode, Result, Versioned};
    use uuid::Uuid;

    #[tokio::test]
    async fn test_create_profile_success() -> Result<()> {
        // Arrange
        let f = ProfileTestFixture::new();

        // On génère un contexte de création légitime (profile_id à None) via l'usine de la fixture
        let creation_ctx = f.app_ctx().create_creation_context(f.region());

        let cmd = CreateProfileCommand {
            command_id: Uuid::new_v4(),
            account_id: f.account_id().clone(),
            handle: Handle::try_new("bob_dev")?,
            region: f.region(),
        };

        // Act - On passe le contexte de création unifié
        f.bus()
            .execute::<ProfileContext, CreateProfileCommand, ()>(creation_ctx, cmd.clone())
            .await?;

        // Assert
        let saved_profile = f
            .profile_repo()
            .find_by_handle(&cmd.handle, &f.region(), None)
            .await?
            .expect("Le profil aurait dû être enregistré en base");

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

        // 1. On simule que la commande a déjà été exécutée avec succès au premier essai
        f.idempotency_repo().seed(cmd_id);

        // 2. On insère un profil qui correspond à ce premier essai réussi
        let existing_profile = f.builder("bob_dev").build()?;
        f.profile_repo().save_direct(existing_profile).await;

        let cmd = CreateProfileCommand {
            command_id: cmd_id, // Même ID de commande -> Va forcer le court-circuit
            account_id: f.account_id().clone(),
            handle: Handle::try_new("bob_dev")?,
            region: f.region(),
        };

        // Act
        let result = f
            .bus()
            .execute::<ProfileContext, CreateProfileCommand, ()>(f.profile_ctx().clone(), cmd)
            .await;

        // Assert
        assert!(
            result.is_ok(),
            "Le retry technique d'une création doit être transparent et renvoyer Ok(())"
        );

        // Crucial : On s'assure qu'aucun événement n'a été dupliqué dans l'outbox
        f.assert_outbox(0, None);

        Ok(())
    }

    #[tokio::test]
    async fn test_create_profile_conflict_handle() -> Result<()> {
        // Arrange
        let f = ProfileTestFixture::new();
        let duplicated_handle = "taken_handle";

        // 1. On sème un profil en base qui possède déjà ce handle
        // (Peu importe l'ID, l'unicité du handle est transverse sur la région)
        let other_profile_id = ProfileId::generate();
        let f_other = f.clone_with_profile_id(other_profile_id);

        let profile_with_handle = f_other.builder(duplicated_handle).build()?;
        f_other
            .profile_repo()
            .save_direct(profile_with_handle)
            .await;

        // 2. On tente de créer un NOUVEAU profil avec le même handle usurpé
        let cmd = CreateProfileCommand {
            command_id: Uuid::new_v4(),
            account_id: f.account_id().clone(),
            handle: Handle::try_new(duplicated_handle)?,
            region: f.region(),
        };

        // Act
        let result = f
            .bus()
            .execute::<ProfileContext, CreateProfileCommand, ()>(f.profile_ctx().clone(), cmd)
            .await;

        // Assert
        assert!(
            matches!(result, Err(e) if e.code == ErrorCode::AlreadyExists),
            "Tenter d'utiliser un Handle déjà pris dans la même région doit lever un AlreadyExists"
        );

        Ok(())
    }
}
