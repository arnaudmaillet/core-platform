#[cfg(test)]
mod tests {
    use std::str::FromStr;
    use std::sync::{Arc, Mutex};
    use shared_kernel::domain::entities::GeoPoint;
    use shared_kernel::domain::value_objects::{AccountId, RegionCode};
    use crate::application::get_nearby_users::{GetNearbyUsersCommand, GetNearbyUsersUseCase};
    use crate::domain::builders::UserLocationBuilder;
    use crate::domain::entities::UserLocation;
    use crate::utils::LocationRepositoryStub;

    fn create_mock_loc(lat: f64, lon: f64, privacy_radius: i32) -> UserLocation {
        UserLocationBuilder::new(
            AccountId::new(),
            RegionCode::from_raw("eu"),
            GeoPoint::try_new(lat, lon).unwrap(),
        )
            .with_privacy(false, privacy_radius)
            .build()
    }

    #[tokio::test]
    async fn test_get_nearby_users_nominal_path() {
        // Arrange
        let alice = create_mock_loc(48.8567, 2.3523, 0);
        let bob = create_mock_loc(48.8570, 2.3525, 0);

        let repo = Arc::new(LocationRepositoryStub::default());
        {
            let mut nearby = repo.nearby_to_return.lock().unwrap();
            nearby.push((alice, 100.0));
            nearby.push((bob, 500.0));
        }

        let use_case = GetNearbyUsersUseCase::new(repo);
        let cmd = GetNearbyUsersCommand {
            account_id: AccountId::new(), // ID de l'appelant différent
            center: GeoPoint::try_new(48.8566, 2.3522).unwrap(),
            region: RegionCode::from_raw("eu"),
            radius_meters: 1000.0,
            limit: 10,
        };

        // Act
        let result = use_case.execute(cmd).await.unwrap();

        // Assert
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].distance_meters, 100.0);
    }

    #[tokio::test]
    async fn test_get_nearby_users_excludes_self() {
        let myself = create_mock_loc(48.0, 2.0, 0);
        let my_id = myself.account_id().clone();

        let repo = Arc::new(LocationRepositoryStub::default());
        repo.nearby_to_return.lock().unwrap().push((myself, 0.0));

        let use_case = GetNearbyUsersUseCase::new(repo);
        let cmd = GetNearbyUsersCommand {
            account_id: my_id,
            center: GeoPoint::try_new(48.0, 2.0).unwrap(),
            region: RegionCode::from_raw("eu"),
            radius_meters: 1000.0,
            limit: 10,
        };

        let result = use_case.execute(cmd).await.unwrap();

        // Assert
        assert!(result.is_empty(), "Le Use Case doit filtrer l'ID de l'appelant");
    }

    #[tokio::test]
    async fn test_get_nearby_users_obfuscation_logic() {
        // Arrange
        let original_coords = GeoPoint::try_new(48.8566, 2.3522).unwrap();
        let alice = create_mock_loc(original_coords.lat(), original_coords.lon(), 500);

        let repo = Arc::new(LocationRepositoryStub::default());
        repo.nearby_to_return.lock().unwrap().push((alice, 100.0));

        let use_case = GetNearbyUsersUseCase::new(repo);
        let cmd = GetNearbyUsersCommand {
            account_id: AccountId::new(),
            center: original_coords,
            region: RegionCode::from_raw("eu"),
            radius_meters: 1000.0,
            limit: 10,
        };

        // Act
        let result = use_case.execute(cmd).await.unwrap();

        // Assert
        let res = &result[0];
        assert!(res.is_obfuscated);
        // L'algorithme doit avoir généré de nouvelles coordonnées
        assert_ne!(res.coordinates.lat(), original_coords.lat());
    }

    #[tokio::test]
    async fn test_obfuscate_location_distribution_randomness() {
        // Arrange
        let repo = Arc::new(LocationRepositoryStub::default());
        let use_case = GetNearbyUsersUseCase::new(repo);
        let point = GeoPoint::try_new(45.0, 5.0).unwrap();
        let mut rng = rand::thread_rng();

        // Act
        let mut samples = Vec::new();
        for _ in 0..100 {
            // Désormais accessible grâce à pub(crate)
            samples.push(use_case.obfuscate_location(&point, 500, &mut rng));
        }

        // Assert
        let first = samples[0];
        let identicals = samples.iter().filter(|p| p.lat() == first.lat() && p.lon() == first.lon()).count();

        // On s'attend à ce que l'aléatoire fonctionne (très peu de chance de retomber sur le même point)
        assert!(identicals < 2, "L'algorithme d'obfuscation ne semble pas assez aléatoire");
    }
}