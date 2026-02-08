use crate::domain::entities::UserLocation; // Vérifie bien le nom de ton entité
use crate::domain::repositories::LocationRepository;
use shared_kernel::domain::entities::GeoPoint;
use shared_kernel::domain::transaction::Transaction;
use shared_kernel::domain::value_objects::{AccountId, RegionCode};
use shared_kernel::errors::Result;
use std::sync::Mutex;

// --- STUB LOCATION REPOSITORY ---
pub struct LocationRepositoryStub {
    pub location_to_return: Mutex<Option<UserLocation>>,
    pub error_to_return: Mutex<Option<shared_kernel::errors::DomainError>>,
    pub nearby_to_return: Mutex<Vec<(UserLocation, f64)>>,
}

impl Default for LocationRepositoryStub {
    fn default() -> Self {
        Self {
            location_to_return: Mutex::new(None),
            error_to_return: Mutex::new(None),
            nearby_to_return: Mutex::new(vec![]),
        }
    }
}

#[async_trait::async_trait]
impl LocationRepository for LocationRepositoryStub {
    async fn save(&self, _loc: &UserLocation, _tx: Option<&mut dyn Transaction>) -> Result<()> {
        if let Some(err) = self.error_to_return.lock().unwrap().clone() {
            return Err(err);
        }
        Ok(())
    }

    async fn fetch(&self, _id: &AccountId, _r: &RegionCode) -> Result<Option<UserLocation>> {
        Ok(self.location_to_return.lock().unwrap().clone())
    }

    async fn fetch_nearby(
        &self,
        _center: GeoPoint,
        _region: RegionCode,
        _radius: f64,
        _limit: i64,
    ) -> Result<Vec<(UserLocation, f64)>> {
        Ok(self.nearby_to_return.lock().unwrap().clone())
    }

    async fn delete(&self, _id: &AccountId, _r: &RegionCode) -> Result<()> {
        Ok(())
    }
}
