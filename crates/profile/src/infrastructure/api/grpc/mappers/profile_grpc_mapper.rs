// crates/profile/src/infrastructure/api/grpc/mappers/profile_grpc_mapper.rs

use super::super::profile_v1::Profile as ProtoProfile;
use crate::domain::entities::Profile;
use crate::domain::value_objects::{Bio, DisplayName, SocialLinks, ProfileId, Handle};
use crate::infrastructure::api::grpc::mappers::to_timestamp;
use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::domain::value_objects::{AccountId, LocationLabel, RegionCode, Url};
use shared_kernel::errors::DomainError;

impl From<Profile> for ProtoProfile {
    fn from(domain: Profile) -> Self {
        Self {
            profile_id: domain.id().to_string(),
            owner_id: domain.owner_id().to_string(),
            region_code: domain.region_code().to_string(),
            handle: domain.handle().to_string(),
            display_name: domain.display_name().to_string(),
            bio: domain.bio().map(|b| b.to_string()),
            avatar_url: domain.avatar_url().map(|u| u.to_string()),
            banner_url: domain.banner_url().map(|u| u.to_string()),
            location_label: domain.location_label().map(|l| l.to_string()),
            social_links: domain.social_links().map(|s| s.clone().into()),
            stats: Some(domain.stats().clone().into()),
            post_count: domain.post_count() as u64,
            is_private: domain.is_private(),
            created_at: Some(to_timestamp(domain.created_at())),
            updated_at: Some(to_timestamp(domain.updated_at())),
            version: domain.version(),
        }
    }
}

impl TryFrom<ProtoProfile> for Profile {
    type Error = DomainError;

    fn try_from(proto: ProtoProfile) -> Result<Self, Self::Error> {
        let profile_id = ProfileId::try_from(proto.profile_id)?;
        let owner_id = AccountId::try_from(proto.owner_id)?;
        let region_code = RegionCode::try_new(proto.region_code)?;
        let handle = Handle::try_new(proto.handle)?;
        let display_name = DisplayName::try_new(proto.display_name)?;

        let social_links = proto.social_links
            .map(SocialLinks::try_from)
            .transpose()?;

        // Utilisation de restore() pour préserver l'état (ID et Version)
        Ok(Profile::restore(
            profile_id,
            owner_id,
            region_code,
            display_name,
            handle,
            proto.bio.filter(|s| !s.trim().is_empty()).map(Bio::from_raw),
            proto.avatar_url.filter(|s| !s.trim().is_empty()).map(Url::from_raw),
            proto.banner_url.filter(|s| !s.trim().is_empty()).map(Url::from_raw),
            proto.location_label.filter(|s| !s.trim().is_empty()).map(LocationLabel::from_raw),
            social_links,
            proto.post_count.try_into().map_err(|_| DomainError::Validation {
                field: "post_count",
                reason: "Invalid count".into()
            })?,
            proto.is_private,
            proto.version,
            chrono::Utc::now(),
            chrono::Utc::now(),
        ))
    }
}