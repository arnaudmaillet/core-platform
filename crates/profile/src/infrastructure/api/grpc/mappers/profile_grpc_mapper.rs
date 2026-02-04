// crates/profile/src/infrastructure/api/grpc/mappers/profile_grpc_mapper.rs

use super::super::profile_v1::Profile as ProtoProfile;
use crate::domain::builders::ProfileBuilder;
use crate::domain::entities::Profile;
use crate::domain::value_objects::{Bio, DisplayName, SocialLinks};
use crate::infrastructure::api::grpc::mappers::to_timestamp;
use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::domain::value_objects::{AccountId, RegionCode, Url, Username};
use shared_kernel::errors::DomainError;

impl From<Profile> for ProtoProfile {
    fn from(domain: Profile) -> Self {
        Self {
            account_id: domain.account_id().to_string(),
            region_code: domain.region_code().to_string(),
            username: domain.username().to_string(),
            display_name: domain.display_name().to_string(),
            bio: domain.bio().map(|b| b.to_string().into()),
            avatar_url: domain.avatar_url().map(|u| u.to_string().into()),
            banner_url: domain.banner_url().map(|u| u.to_string().into()),
            location_label: domain.location_label().map(|l| l.to_string().into()),
            social_links: domain.social_links().map(|s| s.clone().into()),
            stats: Some(domain.stats().clone().into()),
            post_count: domain.post_count() as i64,
            is_private: domain.is_private(),
            created_at: Some(to_timestamp(domain.created_at())),
            updated_at: Some(to_timestamp(domain.updated_at())),
            version: domain.metadata().version() as i64,
        }
    }
}

impl TryFrom<ProtoProfile> for Profile {
    type Error = DomainError;

    fn try_from(proto: ProtoProfile) -> Result<Self, Self::Error> {
        let account_id = AccountId::try_from(proto.account_id)?;
        let region_code = RegionCode::try_from(proto.region_code)?;
        let username = Username::try_from(proto.username)?;
        let display_name = DisplayName::try_from(proto.display_name)?;
        let social_links = proto.social_links.map(SocialLinks::try_from).transpose()?;

        let builder = ProfileBuilder::new(account_id, region_code, display_name, username)
            .with_privacy(proto.is_private)
            .with_optional_bio(
                proto
                    .bio
                    .filter(|s| !s.trim().is_empty())
                    .map(Bio::try_from)
                    .transpose()?,
            )
            .with_optional_avatar_url(
                proto
                    .avatar_url
                    .filter(|s| !s.trim().is_empty())
                    .map(Url::try_from)
                    .transpose()?,
            )
            .with_optional_banner_url(
                proto
                    .banner_url
                    .filter(|s| !s.trim().is_empty())
                    .map(Url::try_from)
                    .transpose()?,
            )
            .with_optional_social_links(social_links);

        Ok(builder.build())
    }
}
