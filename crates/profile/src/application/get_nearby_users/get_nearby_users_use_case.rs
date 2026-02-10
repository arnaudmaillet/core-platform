// crates/profile/src/application/use_cases/get_nearby_users/mod.rs

use crate::application::get_nearby_users::{GetNearbyUsersCommand, NearbyUserDto};
use crate::domain::repositories::LocationRepository;
use rand::Rng;
use shared_kernel::domain::entities::GeoPoint;
use shared_kernel::errors::Result;
use std::sync::Arc;

pub struct GetNearbyUsersUseCase {
    repo: Arc<dyn LocationRepository>,
}

impl GetNearbyUsersUseCase {
    pub fn new(repo: Arc<dyn LocationRepository>) -> Self {
        Self { repo }
    }

    pub async fn execute(&self, cmd: GetNearbyUsersCommand) -> Result<Vec<NearbyUserDto>> {
        // 1. Appel au repository PostGIS (filtre déjà le ghost_mode = false)
        let raw_results = self
            .repo
            .fetch_nearby(cmd.center, cmd.region, cmd.radius_meters, cmd.limit)
            .await?;

        let mut dtos = Vec::new();
        let mut rng = rand::thread_rng();

        for (loc, distance) in raw_results {
            // Ne pas s'inclure soi-même dans les résultats
            if loc.profile_id().clone() == cmd.profile_id {
                continue;
            }

            // 2. Application de la logique de Privacy (Obfuscation)
            let (final_coords, is_obfuscated) = if loc.privacy_radius_meters() > 0 {
                // Si l'utilisateur a un rayon de 500m, on déplace ses coordonnées
                (
                    self.obfuscate_location(
                        &loc.coordinates(),
                        loc.privacy_radius_meters(),
                        &mut rng,
                    ),
                    true,
                )
            } else {
                (loc.coordinates().clone(), false)
            };

            dtos.push(NearbyUserDto {
                profile_id: loc.profile_id().clone(),
                coordinates: final_coords,
                distance_meters: distance,
                is_obfuscated,
            });
        }

        Ok(dtos)
    }

    /// Algorithme de floutage : déplace un point de manière aléatoire dans un rayon donné
    pub(crate) fn obfuscate_location(
        &self,
        point: &GeoPoint,
        radius_meters: i32,
        rng: &mut impl Rng,
    ) -> GeoPoint {
        let radius_in_degrees = (radius_meters as f64) / 111320.0;

        let u: f64 = rng.random();
        let v: f64 = rng.random();

        let w = radius_in_degrees * u.sqrt();
        let t = 2.0 * std::f64::consts::PI * v;

        let delta_lat = w * t.cos();
        let delta_lon = w * t.sin() / point.lat().to_radians().cos();

        // On utilise unwrap_or car si le floutage nous fait sortir de la terre (très improbable),
        // on préfère renvoyer le point original plutôt que de faire crasher le thread.
        GeoPoint::try_new(point.lat() + delta_lat, point.lon() + delta_lon).unwrap_or(*point)
    }
}
