// crates/profile/tests/application/update_username_use_case_it.rs

#[cfg(test)]
mod tests {
    use crate::application::update_username::{UpdateUsernameCommand, UpdateUsernameUseCase};
    use crate::domain::entities::Profile;
    use crate::domain::value_objects::DisplayName;
    use std::sync::{Arc, Mutex};
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::repositories::OutboxRepositoryStub;
    use shared_kernel::domain::transaction::StubTxManager;
    use shared_kernel::domain::value_objects::{AccountId, RegionCode, Username};
    use shared_kernel::errors::DomainError;
    use crate::domain::repositories::ProfileRepositoryStub;

    /// Helper pour initialiser le Use Case avec des données de test
    fn setup(profile: Option<Profile>, exists: bool) -> UpdateUsernameUseCase {
        let repo = Arc::new(ProfileRepositoryStub {
            profile_to_return: Mutex::new(profile),
            exists_return: Mutex::new(exists),
            ..Default::default()
        });

        UpdateUsernameUseCase::new(
            repo,
            Arc::new(OutboxRepositoryStub::new()),
            Arc::new(StubTxManager)
        )
    }

    #[tokio::test]
    async fn test_update_username_success() {
        // Arrange
        let account_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();
        let initial_profile = Profile::builder(
            account_id.clone(),
            region.clone(),
            DisplayName::from_raw("Bob"),
            Username::try_new("old_bob").unwrap(),
        )
            .build();

        let use_case = setup(Some(initial_profile), false);

        let cmd = UpdateUsernameCommand {
            account_id: account_id.clone(),
            region: region.clone(),
            new_username: Username::try_new("new_bob").unwrap(),
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok(), "Le Use Case devrait réussir");
        let updated_profile = result.unwrap();
        assert_eq!(updated_profile.username().as_str(), "new_bob");
        assert_eq!(updated_profile.version(), 2); // Init(1) -> Updated(2)
    }

    #[tokio::test]
    async fn test_update_username_fails_on_region_mismatch() {
        // Arrange : Profil en EU, Commande en US
        let account_id = AccountId::new();
        let actual_region = RegionCode::try_new("eu").unwrap();
        let wrong_region = RegionCode::try_new("us").unwrap();

        let profile = Profile::builder(
            account_id.clone(),
            actual_region,
            DisplayName::from_raw("Alice"),
            Username::try_new("alice").unwrap(),
        ).build();

        let use_case = setup(Some(profile), false);

        let cmd = UpdateUsernameCommand {
            account_id,
            region: wrong_region,
            new_username: Username::try_new("new_alice").unwrap(),
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert : Doit être bloqué par l'entité (Security Bound)
        assert!(matches!(result, Err(DomainError::Forbidden { .. })));
    }

    #[tokio::test]
    async fn test_update_username_already_exists() {
        // Arrange
        let account_id = AccountId::new();
        let region = RegionCode::try_new("us").unwrap();
        let initial_profile = Profile::builder(
            account_id.clone(),
            region.clone(),
            DisplayName::from_raw("Alice"),
            Username::try_new("alice_orig").unwrap(),
        )
            .build();

        // On simule que le pseudo cible est DÉJÀ pris via le paramètre 'exists' du setup
        let use_case = setup(Some(initial_profile), true);

        let cmd = UpdateUsernameCommand {
            account_id,
            region,
            new_username: Username::try_new("already_taken").unwrap(),
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        match result {
            Err(DomainError::AlreadyExists { field, .. }) => assert_eq!(field, "username"),
            _ => panic!(
                "Devrait retourner une erreur AlreadyExists, reçu: {:?}",
                result
            ),
        }
    }

    #[tokio::test]
    async fn test_update_username_profile_not_found() {
        // Arrange : Aucun profil en DB
        let use_case = setup(None, false);

        let cmd = UpdateUsernameCommand {
            account_id: AccountId::new(),
            region: RegionCode::try_new("eu").unwrap(),
            new_username: Username::try_new("new_name").unwrap(),
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(matches!(result, Err(DomainError::NotFound { .. })));
    }

    #[tokio::test]
    async fn test_update_username_idempotency() {
        // Arrange
        let account_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();
        let current_username = Username::try_new("no_change").unwrap();

        let initial_profile = Profile::builder(
            account_id.clone(),
            region.clone(),
            DisplayName::from_raw("Bob"),
            current_username.clone(),
        )
            .build();

        let use_case = setup(Some(initial_profile), false);

        // Act : On envoie le même pseudo
        let cmd = UpdateUsernameCommand {
            account_id,
            region,
            new_username: current_username,
        };

        let result = use_case.execute(cmd).await;

        // Assert : Succès mais l'entité doit renvoyer false et la version ne doit pas changer
        assert!(result.is_ok());
        let updated = result.unwrap();
        assert_eq!(updated.username().as_str(), "no_change");
        assert_eq!(updated.version(), 1);
    }
}