// crates/profile/tests/application/update_handle_use_case_it.rs

#[cfg(test)]
mod tests {
    use crate::application::update_handle::{UpdateHandleCommand, UpdateHandleUseCase};
    use crate::domain::entities::Profile;
    use crate::domain::value_objects::{DisplayName, Handle, ProfileId}; // Ajout Handle et ProfileId
    use std::sync::{Arc, Mutex};
    use shared_kernel::domain::events::AggregateRoot;
    use shared_kernel::domain::repositories::OutboxRepositoryStub;
    use shared_kernel::domain::transaction::StubTxManager;
    use shared_kernel::domain::value_objects::{AccountId, RegionCode};
    use shared_kernel::errors::DomainError;
    use crate::domain::repositories::ProfileRepositoryStub;

    /// Helper pour initialiser le Use Case avec des données de test
    fn setup(profile: Option<Profile>, exists: bool) -> UpdateHandleUseCase {
        let repo = Arc::new(ProfileRepositoryStub {
            profile_to_return: Mutex::new(profile),
            exists_return: Mutex::new(exists),
            ..Default::default()
        });

        UpdateHandleUseCase::new(
            repo,
            Arc::new(OutboxRepositoryStub::new()),
            Arc::new(StubTxManager)
        )
    }

    #[tokio::test]
    async fn test_update_handle_success() {
        // Arrange
        let owner_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();
        let initial_profile = Profile::builder(
            owner_id,
            region.clone(),
            DisplayName::from_raw("Bob"),
            Handle::try_new("old_bob").unwrap(), // Username -> Handle
        )
            .build();

        let profile_id = initial_profile.id().clone();
        let use_case = setup(Some(initial_profile), false);

        let cmd = UpdateHandleCommand {
            profile_id: profile_id.clone(),
            region: region.clone(),
            new_handle: Handle::try_new("new_bob").unwrap(),
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok(), "Le Use Case devrait réussir");
        let updated_profile = result.unwrap();
        assert_eq!(updated_profile.handle().as_str(), "new_bob");
        assert_eq!(updated_profile.version(), 2);
    }

    #[tokio::test]
    async fn test_update_handle_fails_on_region_mismatch() {
        // Arrange : Profil en EU, Commande en US
        let owner_id = AccountId::new();
        let actual_region = RegionCode::try_new("eu").unwrap();
        let wrong_region = RegionCode::try_new("us").unwrap();

        let profile = Profile::builder(
            owner_id,
            actual_region,
            DisplayName::from_raw("Alice"),
            Handle::try_new("alice").unwrap(),
        ).build();

        let profile_id = profile.id().clone();
        let use_case = setup(Some(profile), false);

        let cmd = UpdateHandleCommand {
            profile_id,
            region: wrong_region,
            new_handle: Handle::try_new("new_alice").unwrap(),
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(matches!(result, Err(DomainError::Forbidden { .. })));
    }

    #[tokio::test]
    async fn test_update_handle_already_exists() {
        // Arrange
        let owner_id = AccountId::new();
        let region = RegionCode::try_new("us").unwrap();
        let initial_profile = Profile::builder(
            owner_id,
            region.clone(),
            DisplayName::from_raw("Alice"),
            Handle::try_new("alice_orig").unwrap(),
        )
            .build();

        let profile_id = initial_profile.id().clone();
        let use_case = setup(Some(initial_profile), true);

        let cmd = UpdateHandleCommand {
            profile_id,
            region,
            new_handle: Handle::try_new("already_taken").unwrap(),
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        match result {
            Err(DomainError::AlreadyExists { field, .. }) => assert_eq!(field, "handle"),
            _ => panic!("Attendu: AlreadyExists(handle), Reçu: {:?}", result),
        }
    }

    #[tokio::test]
    async fn test_update_handle_profile_not_found() {
        // Arrange
        let use_case = setup(None, false);

        let cmd = UpdateHandleCommand {
            profile_id: ProfileId::new(),
            region: RegionCode::try_new("eu").unwrap(),
            new_handle: Handle::try_new("new_name").unwrap(),
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(matches!(result, Err(DomainError::NotFound { .. })));
    }

    #[tokio::test]
    async fn test_update_handle_idempotency() {
        // Arrange
        let owner_id = AccountId::new();
        let region = RegionCode::try_new("eu").unwrap();
        let current_handle = Handle::try_new("no_change").unwrap();

        let initial_profile = Profile::builder(
            owner_id,
            region.clone(),
            DisplayName::from_raw("Bob"),
            current_handle.clone(),
        )
            .build();

        let profile_id = initial_profile.id().clone();
        let use_case = setup(Some(initial_profile), false);

        let cmd = UpdateHandleCommand {
            profile_id,
            region,
            new_handle: current_handle,
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        let updated = result.unwrap();
        assert_eq!(updated.handle().as_str(), "no_change");
        assert_eq!(updated.version(), 1); // Pas d'incrément car pas de changement
    }
}