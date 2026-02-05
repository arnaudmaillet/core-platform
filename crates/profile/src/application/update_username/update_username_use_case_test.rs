// crates/profile/tests/application/update_username_use_case_it.rs

#[cfg(test)]
mod tests {
    use crate::application::update_username::{UpdateUsernameCommand, UpdateUsernameUseCase};
    use crate::domain::entities::Profile;
    use crate::domain::value_objects::DisplayName;
    use std::sync::{Arc, Mutex};
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
            error_to_return: Mutex::new(None),
        });

        UpdateUsernameUseCase::new(repo, Arc::new(OutboxRepositoryStub::new()), Arc::new(StubTxManager))
    }

    #[tokio::test]
    async fn test_update_username_success() {
        // Arrange
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");
        let initial_profile = Profile::builder(
            account_id.clone(),
            region.clone(),
            DisplayName::from_raw("Bob"),
            Username::try_new("old_bob").unwrap(),
        )
        .build();

        let use_case = setup(Some(initial_profile), false);

        let cmd = UpdateUsernameCommand {
            account_id,
            region,
            new_username: Username::try_new("new_bob").unwrap(),
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok(), "Le Use Case devrait réussir");
        let updated_profile = result.unwrap();
        assert_eq!(updated_profile.username().as_str(), "new_bob");
    }

    #[tokio::test]
    async fn test_update_username_already_exists() {
        // Arrange
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("us");
        let initial_profile = Profile::builder(
            account_id.clone(),
            region.clone(),
            DisplayName::from_raw("Alice"),
            Username::try_new("alice_orig").unwrap(),
        )
        .build();

        // On simule que le pseudo cible est DEJÀ pris
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
        // Arrange : Aucun profil en DB (None)
        let use_case = setup(None, false);

        let cmd = UpdateUsernameCommand {
            account_id: AccountId::new(),
            region: RegionCode::from_raw("eu"),
            new_username: Username::try_new("new_name").unwrap(),
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(matches!(result, Err(DomainError::NotFound { .. })));
    }

    #[tokio::test]
    async fn test_update_username_no_change_needed() {
        // Arrange
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");
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

        // Assert : Succès mais logiquement rien ne doit changer
        assert!(result.is_ok());
        assert_eq!(result.unwrap().username().as_str(), "no_change");
    }
}
