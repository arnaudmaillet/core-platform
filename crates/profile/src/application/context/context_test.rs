// crates/profile/src/application/context.rs (ou fichier de tests dédié au contexte)

#[cfg(test)]
mod tests {
    use crate::application::utils::ProfileTestFixture;
    use shared_kernel::command::CommandTarget;
    use shared_kernel::core::{ErrorCode, Result, Versioned};
    use shared_kernel::types::ProfileId;

    #[tokio::test]
    async fn test_context_fetch_verified_occ_conflict() -> Result<()> {
        // Arrange
        let f = ProfileTestFixture::new();
        let profile = f.builder("alice").build()?;
        let current_version = profile.version();
        f.given_profile(profile).await;

        // On crée une version qui est forcément différente pour déclencher l'OCC
        let wrong_version = current_version + 1;
        let target = CommandTarget::new(f.profile_id().clone(), f.region(), wrong_version);

        // Act
        let result = f.profile_ctx().fetch_verified(&target).await;

        // Assert
        assert!(
            matches!(
                result,
                Err(e) if e.code == ErrorCode::ConcurrencyConflict
            ),
            "Une désynchronisation de version doit lever un ConcurrencyConflict"
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_context_save_identity_guardrail() -> Result<()> {
        // Arrange
        let f = ProfileTestFixture::new();
        let mut profile = f.builder("alice").build()?;

        // NETTOYAGE PLUS DE HACK MUTABLE :
        // On génère un ID totalement distinct
        let mismatched_id = ProfileId::generate();

        // On utilise l'usine officielle de la fixture pour créer un contexte
        // lié à cet ID distinct. C'est propre, immuable et réaliste.
        let corrupted_context = f.app_ctx().create_context(mismatched_id, f.region());

        // Act
        let result = corrupted_context.save(&mut profile, None).await;

        // Assert
        // Doit rejeter la sauvegarde car le profil a un ID différent du contexte
        match result {
            Err(e) => {
                assert_eq!(e.code, ErrorCode::ValidationFailed);
                assert!(e.message.contains("profile_id"));
            }
            Ok(_) => panic!(
                "Le guardrail d'identité aurait dû bloquer la sauvegarde avec un ValidationFailed"
            ),
        }
        Ok(())
    }

    #[tokio::test]
    async fn test_context_idempotency_check() -> Result<()> {
        // Arrange
        let f = ProfileTestFixture::new();
        let mut profile = f.builder("alice").build()?;
        f.given_profile(profile.clone()).await;

        let cmd_id = uuid::Uuid::new_v4();
        // On simule que la commande a déjà été enregistrée dans la transaction d'idempotence
        f.idempotency_repo().seed(cmd_id);

        // Act
        let result = f.profile_ctx().save(&mut profile, Some(cmd_id)).await;

        // Assert
        match result {
            Err(e) => assert_eq!(e.code, ErrorCode::AlreadyExists),
            Ok(_) => panic!(
                "Le contexte devrait détecter le doublon en base et lever un AlreadyExists pour le Bus"
            ),
        }
        Ok(())
    }
}
