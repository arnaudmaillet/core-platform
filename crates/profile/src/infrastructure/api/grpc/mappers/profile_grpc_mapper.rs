// crates/profile/src/infrastructure/api/grpc/mappers/profile_grpc_mapper.rs

use shared_kernel::domain::value_objects::{AccountId, RegionCode, Url, Username};
use crate::domain::builders::ProfileBuilder;
use crate::domain::entities::Profile;
use crate::domain::value_objects::{Bio, DisplayName};
use crate::infrastructure::api::grpc::mappers::to_timestamp;
use super::super::profile_v1::{
    Profile as ProtoProfile,
};

impl From<Profile> for ProtoProfile {
    fn from(domain: Profile) -> Self {
        Self {
            account_id: domain.account_id.to_string(),
            region_code: domain.region_code.to_string(),
            username: domain.username.to_string(),
            display_name: domain.display_name.to_string(),
            bio: domain.bio.map(|b| b.to_string().into()),
            avatar_url: domain.avatar_url.map(|u| u.to_string().into()),
            banner_url: domain.banner_url.map(|u| u.to_string().into()),
            location_label: domain.location_label.map(|l| l.to_string().into()),
            social_links: domain.social_links.map(|s| s.into()),
            stats: Some(domain.stats.into()),
            post_count: domain.post_count.value() as i64,
            is_private: domain.is_private,
            created_at: Some(to_timestamp(domain.created_at)),
            updated_at: Some(to_timestamp(domain.updated_at)),
            version: domain.metadata.version as i64,
        }
    }
}

impl TryFrom<ProtoProfile> for Profile {
    type Error = String;

    fn try_from(proto: ProtoProfile) -> Result<Self, Self::Error> {
        let account_id = AccountId::try_from(proto.account_id).map_err(|e| e.to_string())?;
        let region_code = RegionCode::try_from(proto.region_code).map_err(|e| e.to_string())?;
        let username = Username::try_from(proto.username).map_err(|e| e.to_string())?;
        let display_name = DisplayName::try_from(proto.display_name).map_err(|e| e.to_string())?;

        let builder = ProfileBuilder::new(account_id, region_code, display_name, username)
            .is_private(proto.is_private)
            .maybe_bio(proto.bio.filter(|s| !s.trim().is_empty()).map(Bio::try_from).transpose().map_err(|e| e.to_string())?)
            .maybe_avatar_url(proto.avatar_url.filter(|s| !s.trim().is_empty()).map(Url::try_from).transpose().map_err(|e| e.to_string())?)
            .maybe_banner_url(proto.banner_url.filter(|s| !s.trim().is_empty()).map(Url::try_from).transpose().map_err(|e| e.to_string())?)
            .maybe_social_links(proto.social_links.map(|s| s.into()));

        Ok(builder.build())
    }
}