#[cfg(test)]
mod tests {
    use crate::application::update_location::{UpdateLocationCommand, UpdateLocationUseCase};
    use crate::domain::builders::UserLocationBuilder;
    use crate::domain::entities::UserLocation;
    use chrono::{Duration, Utc};
    use shared_kernel::domain::entities::GeoPoint;
    use shared_kernel::domain::value_objects::RegionCode;
    use shared_kernel::errors::{DomainError, Result};
    use std::sync::{Arc, Mutex};
    use shared_kernel::domain::events::EventEnvelope;
    use shared_kernel::domain::repositories::OutboxRepositoryStub;
    use shared_kernel::domain::transaction::StubTxManager;
    use crate::domain::repositories::LocationRepositoryStub;
    use crate::domain::value_objects::ProfileId;

    fn setup(location: Option<UserLocation>) -> UpdateLocationUseCase {
        let repo = Arc::new(LocationRepositoryStub {
            location_to_return: Mutex::new(location),
            ..Default::default()
        });

        UpdateLocationUseCase::new(repo, Arc::new(OutboxRepositoryStub::new()), Arc::new(StubTxManager))
    }

    #[tokio::test]
    async fn test_update_location_success() {
        // Arrange : Position initiale à Paris
        let profile_id = ProfileId::new();
        let region = RegionCode::from_raw("eu");
        let initial_coords = GeoPoint::from_raw(48.8566, 2.3522); // Paris
        let initial_location =
            UserLocationBuilder::new(profile_id.clone(), region.clone(), initial_coords).build();

        let use_case = setup(Some(initial_location));

        // Nouvelle position à plus de 5 mètres (ex: Lyon)
        let new_coords = GeoPoint::from_raw(45.7640, 4.8357);
        let cmd = UpdateLocationCommand {
            profile_id,
            region,
            coords: new_coords.clone(),
            metrics: None,
            movement: None,
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        // Ici, on vérifierait normalement via un Spy ou un Mock que repo.save() a été appelé
    }

    #[tokio::test]
    async fn test_throttling_insignificant_movement() {
        // Arrange : Position initiale
        let profile_id = ProfileId::new();
        let region = RegionCode::from_raw("eu");
        let initial_coords = GeoPoint::from_raw(48.8566, 2.3522);
        let ten_seconds_ago = Utc::now() - Duration::seconds(10);
        let location = UserLocation::restore(
            profile_id.clone(),
            region.clone(),
            initial_coords.clone(),
            None,
            None,
            false,
            0,
            ten_seconds_ago,
            1,
        );

        let use_case = setup(Some(location));

        // Mouvement de seulement 1 mètre
        let tiny_move = GeoPoint::from_raw(48.8566001, 2.3522001);

        let cmd = UpdateLocationCommand {
            profile_id,
            region,
            coords: tiny_move,
            metrics: None,
            movement: None,
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
        // Le test passe sans erreur, mais le domaine a court-circuité (Idempotence)
    }

    #[tokio::test]
    async fn test_update_forced_after_time_limit() {
        // Arrange : Même avec un mouvement nul, si assez de temps est passé, on update
        let profile_id = ProfileId::new();
        let region = RegionCode::from_raw("eu");
        let coords = GeoPoint::from_raw(48.8566, 2.3522);
        let one_minute_ago = Utc::now() - Duration::seconds(60);
        let location = UserLocation::restore(
            profile_id.clone(),
            region.clone(),
            coords.clone(),
            None,
            None,
            false,
            0,
            one_minute_ago,
            1,
        );

        let use_case = setup(Some(location));

        let cmd = UpdateLocationCommand {
            profile_id,
            region,
            coords, // Identique
            metrics: None,
            movement: None,
        };

        // Act
        let result = use_case.execute(cmd).await;

        // Assert
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_update_location_not_found() {
        let use_case = setup(None);
        let cmd = UpdateLocationCommand {
            profile_id: ProfileId::new(),
            region: RegionCode::from_raw("eu"),
            coords: GeoPoint::from_raw(0.0, 0.0),
            metrics: None,
            movement: None,
        };

        let result = use_case.execute(cmd).await;
        assert!(matches!(result, Err(DomainError::NotFound { .. })));
    }

    #[tokio::test]
    async fn test_update_location_concurrency_conflict() {
        let profile_id = ProfileId::new();
        let region = RegionCode::from_raw("eu");
        let location = UserLocationBuilder::new(
            profile_id.clone(),
            region.clone(),
            GeoPoint::from_raw(0.0, 0.0),
        )
        .build();

        // Simulation d'une erreur de version lors du save
        let repo = Arc::new(LocationRepositoryStub {
            location_to_return: Mutex::new(Some(location)),
            error_to_return: Mutex::new(Some(DomainError::ConcurrencyConflict {
                reason: "Version mismatch".into(),
            })),
            ..Default::default()
        });

        let use_case =
            UpdateLocationUseCase::new(repo, Arc::new(OutboxRepositoryStub::new()), Arc::new(StubTxManager));

        let result = use_case
            .execute(UpdateLocationCommand {
                profile_id,
                region,
                coords: GeoPoint::from_raw(1.0, 1.0),
                metrics: None,
                movement: None,
            })
            .await;

        // On vérifie que le conflit remonte (le retry aura été tenté par with_retry)
        assert!(matches!(
            result,
            Err(DomainError::ConcurrencyConflict { .. })
        ));
    }

    #[tokio::test]
    async fn test_update_location_atomic_outbox_failure() {
        let profile_id = ProfileId::new();
        let region = RegionCode::from_raw("eu");
        let location = UserLocationBuilder::new(
            profile_id.clone(),
            region.clone(),
            GeoPoint::from_raw(0.0, 0.0),
        )
        .build();

        struct FailingOutbox;
        #[async_trait::async_trait]
        impl shared_kernel::domain::repositories::OutboxRepository for FailingOutbox {
            async fn save(
                &self,
                _: &mut dyn shared_kernel::domain::transaction::Transaction,
                _: &dyn shared_kernel::domain::events::DomainEvent,
            ) -> Result<()> {
                Err(DomainError::Internal("Outbox error".into()))
            }

            async fn find_pending(&self, _limit: i32) -> Result<Vec<EventEnvelope>> {
                Ok(vec![])
            }
        }

        let use_case = UpdateLocationUseCase::new(
            Arc::new(LocationRepositoryStub {
                location_to_return: Mutex::new(Some(location)),
                ..Default::default()
            }),
            Arc::new(FailingOutbox),
            Arc::new(StubTxManager),
        );

        let result = use_case
            .execute(UpdateLocationCommand {
                profile_id,
                region,
                coords: GeoPoint::from_raw(1.0, 1.0),
                metrics: None,
                movement: None,
            })
            .await;

        // Échec Outbox -> Erreur retournée
        assert!(result.is_err());
    }
}
