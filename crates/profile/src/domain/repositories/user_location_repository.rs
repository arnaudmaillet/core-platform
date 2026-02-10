// crates/profile/src/domain/repositories/location_repository.rs

use crate::domain::entities::UserLocation;
use async_trait::async_trait;
use shared_kernel::domain::entities::GeoPoint;
use shared_kernel::domain::transaction::Transaction;
use shared_kernel::domain::value_objects::RegionCode;
use shared_kernel::errors::Result;
use crate::domain::value_objects::ProfileId;

#[async_trait]
pub trait LocationRepository: Send + Sync {
    /// Sauvegarde ou met à jour la position (UPSERT)
    async fn save(&self, location: &UserLocation, tx: Option<&mut dyn Transaction>) -> Result<()>;

    /// Récupère la position actuelle d'un utilisateur
    async fn fetch(
        &self,
        profile_id: &ProfileId,
        region: &RegionCode,
    ) -> Result<Option<UserLocation>>;

    /// Recherche de proximité : Trouve les utilisateurs dans un rayon donné (en mètres)
    /// Retourne une liste de tuples (UserLocation, distance_en_metres)
    async fn fetch_nearby(
        &self,
        center: GeoPoint,
        region: RegionCode,
        radius_meters: f64,
        limit: i64,
    ) -> Result<Vec<(UserLocation, f64)>>;

    async fn delete(&self, profile_id: &ProfileId, region: &RegionCode) -> Result<()>;
}
