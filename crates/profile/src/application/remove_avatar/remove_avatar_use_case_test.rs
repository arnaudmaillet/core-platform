// crates/profile/src/application/remove_avatar/remove_avatar_use_case_test.rs

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};
    use shared_kernel::domain::events::EventEnvelope;
    use shared_kernel::domain::repositories::OutboxRepositoryStub;
    use shared_kernel::domain::transaction::StubTxManager;
    // Import de notre kit de survie centralisé

    use crate::application::remove_avatar::{RemoveAvatarCommand, RemoveAvatarUseCase};
    use crate::domain::entities::Profile;
    use crate::domain::value_objects::DisplayName;
    
    use shared_kernel::domain::value_objects::{AccountId, RegionCode, Url, Username};
    use shared_kernel::errors::DomainError;
    use crate::domain::repositories::ProfileRepositoryStub;

    /// Helper local pour instancier le Use Case rapidement
    fn setup(profile: Option<Profile>) -> RemoveAvatarUseCase {
        let repo = Arc::new(ProfileRepositoryStub {
            profile_to_return: Mutex::new(profile),
            exists_return: Mutex::new(false),
            error_to_return: Mutex::new(None),
        });

        RemoveAvatarUseCase::new(repo, Arc::new(OutboxRepositoryStub::new()), Arc::new(StubTxManager))
    }

    #[tokio::test]
    async fn test_remove_avatar_success() {
        // Arrange : Un profil qui A un avatar
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");
        let url = Url::from_raw("https://cdn.com/old_photo.png");
        let mut profile = Profile::builder(
            account_id.clone(),
            region.clone(),
            DisplayName::from_raw("Bob"),
            Username::try_new("bob").unwrap(),
        )
        .build();
        profile.update_avatar(url);

        let use_case = setup(Some(profile));
        let cmd = RemoveAvatarCommand { account_id, region };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        let updated_profile = result.unwrap();
        assert!(
            updated_profile.avatar_url().is_none(),
            "L'avatar devrait être supprimé (None)"
        );
    }

    #[tokio::test]
    async fn test_remove_avatar_already_none() {
        // Arrange : Un profil qui n'a DEJA PAS d'avatar
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");
        let profile = Profile::builder(
            account_id.clone(),
            region.clone(),
            DisplayName::from_raw("Bob"),
            Username::try_new("bob").unwrap(),
        )
        .build();
        // Ici, profile.avatar_url est None par défaut

        let use_case = setup(Some(profile));
        let cmd = RemoveAvatarCommand { account_id, region };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        let updated_profile = result.unwrap();
        assert!(updated_profile.avatar_url().is_none());
        // La logique interne 'if !profile.remove_avatar()' a fonctionné
    }

    #[tokio::test]
    async fn test_remove_avatar_profile_not_found() {
        // Arrange : Aucun profil trouvé
        let use_case = setup(None);

        let cmd = RemoveAvatarCommand {
            account_id: AccountId::new(),
            region: RegionCode::from_raw("eu"),
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(matches!(result, Err(DomainError::NotFound { .. })));
    }

    #[tokio::test]
    async fn test_remove_avatar_concurrency_conflict() {
        // Arrange : On simule un profil chargé en v2
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");
        let mut profile = Profile::builder(
            account_id.clone(),
            region.clone(),
            DisplayName::from_raw("Bob"),
            Username::try_new("bob").unwrap(),
        )
        .build();
        profile.update_avatar(Url::from_raw("https://old.png")); // v2

        // On configure le stub pour simuler qu'entre-temps, quelqu'un d'autre a modifié le profil
        let repo = Arc::new(ProfileRepositoryStub {
            profile_to_return: Mutex::new(Some(profile)),
            error_to_return: Mutex::new(Some(DomainError::ConcurrencyConflict {
                reason: "Version mismatch in DB".into(),
            })),
            ..Default::default()
        });

        let use_case =
            RemoveAvatarUseCase::new(repo, Arc::new(OutboxRepositoryStub::new()), Arc::new(StubTxManager));

        // Act
        let result = use_case
            .execute(RemoveAvatarCommand { account_id, region })
            .await;

        // Assert
        assert!(matches!(
            result,
            Err(DomainError::ConcurrencyConflict { .. })
        ));
    }

    #[tokio::test]
    async fn test_remove_avatar_db_internal_error() {
        // Arrange
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");
        let mut profile = Profile::builder(
            account_id.clone(),
            region.clone(),
            DisplayName::from_raw("Bob"),
            Username::try_new("bob").unwrap(),
        )
        .build();
        profile.update_avatar(Url::from_raw("https://old.png"));

        // On simule une coupure réseau/DB au moment du save
        let repo = Arc::new(ProfileRepositoryStub {
            profile_to_return: Mutex::new(Some(profile)),
            error_to_return: Mutex::new(Some(DomainError::Internal(
                "Postgres connection timeout".into(),
            ))),
            ..Default::default()
        });

        let use_case =
            RemoveAvatarUseCase::new(repo, Arc::new(OutboxRepositoryStub::new()), Arc::new(StubTxManager));

        // Act
        let result = use_case
            .execute(RemoveAvatarCommand { account_id, region })
            .await;

        // Assert
        assert!(matches!(result, Err(DomainError::Internal(m)) if m.contains("timeout")));
    }

    #[tokio::test]
    async fn test_remove_avatar_outbox_error_triggers_failure() {
        // Arrange
        let account_id = AccountId::new();
        let region = RegionCode::from_raw("eu");
        let mut profile = Profile::builder(
            account_id.clone(),
            region.clone(),
            DisplayName::from_raw("Bob"),
            Username::try_new("bob").unwrap(),
        )
        .build();
        profile.update_avatar(Url::from_raw("https://old.png"));

        // Création d'un stub Outbox qui échoue systématiquement
        struct FailingOutbox;
        #[async_trait::async_trait]
        impl shared_kernel::domain::repositories::OutboxRepository for FailingOutbox {
            async fn save(
                &self,
                _: &mut dyn shared_kernel::domain::transaction::Transaction,
                _: &dyn shared_kernel::domain::events::DomainEvent,
            ) -> shared_kernel::errors::Result<()> {
                Err(DomainError::Internal("Outbox capacity reached".into()))
            }

            async fn find_pending(&self, _limit: i32) -> shared_kernel::errors::Result<Vec<EventEnvelope>> {
                Ok(vec![])
            }
        }

        let use_case = RemoveAvatarUseCase::new(
            Arc::new(ProfileRepositoryStub {
                profile_to_return: Mutex::new(Some(profile)),
                ..Default::default()
            }),
            Arc::new(FailingOutbox),
            Arc::new(StubTxManager),
        );

        // Act
        let result = use_case
            .execute(RemoveAvatarCommand { account_id, region })
            .await;

        // Assert
        // L'échec de l'outbox doit faire échouer l'ensemble du Use Case (atomique)
        assert!(result.is_err());
    }
}
