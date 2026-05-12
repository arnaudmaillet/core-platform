#[cfg(test)]
mod tests {
    use crate::application::utils::ProfileTestFixture;
    use crate::value_objects::ProfileId;
    use shared_kernel::application::CommandTarget;
    use shared_kernel::domain::entities::Versioned;
    use shared_kernel::errors::{DomainError, Result};

    #[tokio::test]
    async fn test_context_fetch_verified_occ_conflict() -> Result<()> {
        let f = ProfileTestFixture::new();
        let profile = f.builder("alice").build()?;
        let current_version = profile.version();
        f.given_profile(profile).await;

        // On crée une version qui est forcément différente
        let wrong_version = current_version + 1;
        let target = CommandTarget::new(f.profile_id().clone(), f.region(), wrong_version);

        let result = f.profile_ctx().fetch_verified(&target).await;

        assert!(matches!(
            result,
            Err(DomainError::ConcurrencyConflict { .. })
        ));
        Ok(())
    }

    #[tokio::test]
    async fn test_context_save_identity_guardrail() -> Result<()> {
        let f = ProfileTestFixture::new();
        let mut profile = f.builder("alice").build()?;

        // On simule une corruption : on change l'ID de l'objet profil
        // juste avant de sauvegarder dans un contexte lié à un AUTRE ID.
        let other_id = ProfileId::generate();
        let mut corrupted_context = f.profile_ctx().clone();
        corrupted_context.set_profile_id_for_test(other_id);

        let result = corrupted_context.save(&mut profile, None).await;

        assert!(
            matches!(result, Err(DomainError::Validation { field, .. }) if field == "profile_id")
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_context_idempotency_check() -> Result<()> {
        let f = ProfileTestFixture::new();
        let mut profile = f.builder("alice").build()?;
        f.given_profile(profile.clone()).await;

        let cmd_id = uuid::Uuid::new_v4();

        // 1. On "seed" le repo d'idempotence manuellement
        f.idempotency_repo().seed(cmd_id);

        // 2. On essaie de sauvegarder avec ce même ID
        let result = f.profile_ctx().save(&mut profile, Some(cmd_id)).await;

        assert!(
            matches!(result, Err(DomainError::AlreadyExists { entity, .. }) if entity == "Command")
        );
        Ok(())
    }
}
