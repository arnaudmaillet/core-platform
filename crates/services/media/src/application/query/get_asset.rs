use std::sync::Arc;

use cqrs::{Envelope, Query, QueryHandler};

use crate::application::port::AssetRepository;
use crate::domain::aggregate::Asset;
use crate::domain::value_object::AssetId;
use crate::error::MediaError;

/// Fetch an asset's metadata + rendition catalog (no URLs — use `ResolveDelivery`).
#[derive(Debug, Clone)]
pub struct GetAssetQuery {
    pub asset_id: AssetId,
}

impl Query for GetAssetQuery {
    type Response = Asset;
}

pub struct GetAssetHandler {
    assets: Arc<dyn AssetRepository>,
}

impl GetAssetHandler {
    pub fn new(assets: Arc<dyn AssetRepository>) -> Self {
        Self { assets }
    }
}

impl QueryHandler<GetAssetQuery> for GetAssetHandler {
    type Error = MediaError;

    async fn handle(&self, envelope: Envelope<GetAssetQuery>) -> Result<Asset, Self::Error> {
        let id = envelope.payload.asset_id;
        self.assets
            .find_by_id(&id)
            .await?
            .ok_or_else(|| MediaError::AssetNotFound { id: id.as_str() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::fakes::Fixture;
    use crate::domain::value_object::MediaKind;
    use uuid::Uuid;

    #[tokio::test]
    async fn returns_a_stored_asset() {
        let fx = Fixture::new();
        let (asset_id, _owner) = fx.ready_asset(MediaKind::PostImage).await;
        let env = Envelope::new(Uuid::now_v7(), GetAssetQuery { asset_id });
        let asset = fx.get_asset_handler().handle(env).await.unwrap();
        assert_eq!(asset.id(), asset_id);
    }

    #[tokio::test]
    async fn missing_asset_is_not_found() {
        let fx = Fixture::new();
        let env = Envelope::new(Uuid::now_v7(), GetAssetQuery { asset_id: AssetId::new() });
        let err = fx.get_asset_handler().handle(env).await.unwrap_err();
        assert!(matches!(err, MediaError::AssetNotFound { .. }));
    }
}
