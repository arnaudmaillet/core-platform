// crates/profile/src/domain/repositories/location_repository.rs

use async_trait::async_trait;
use shared_kernel::domain::entities::GeoPoint;
use shared_kernel::domain::transaction::Transaction;
use shared_kernel::domain::value_objects::{RegionCode, AccountId};
use shared_kernel::errors::Result;
use crate::domain::entities::UserLocation;

#[async_trait]
pub trait LocationRepository: Send + Sync {
    /// Sauvegarde ou met à jour la position (UPSERT)
    async fn save(&self, location: &UserLocation, tx: Option<&mut dyn Transaction>) -> Result<()>;

    /// Récupère la position actuelle d'un utilisateur
    async fn find_by_id(&self, account_id: &AccountId, region: &RegionCode) -> Result<Option<UserLocation>>;

    /// Recherche de proximité : Trouve les utilisateurs dans un rayon donné (en mètres)
    /// Retourne une liste de tuples (UserLocation, distance_en_metres)
    async fn find_nearby(
        &self,
        center: GeoPoint,
        region: RegionCode,
        radius_meters: f64,
        limit: i64
    ) -> Result<Vec<(UserLocation, f64)>>;

    async fn delete(&self, account_id: &AccountId, region: &RegionCode) -> Result<()>;
}