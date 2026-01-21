// crates/profile/src/infrastructure/api/grpc/mappers/grpc_profile_mapper.rs

use crate::domain::entities::Profile;
use crate::domain::value_objects::{ProfileStats, SocialLinks};
use crate::infrastructure::api::grpc::mappers::grpc_common_mapper::to_timestamp;
use super::super::profile_v1::{
    Profile as ProtoProfile,
    SocialLinks as ProtoSocialLinks,
    ProfileStats as ProtoProfileStats,
    ProfileSummary as ProtoProfileSummary
};

// --- Domaine -> Proto ---

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
            social_links: Some(domain.social_links.into()),
            stats: Some(domain.stats.into()),
            post_count: domain.post_count.value() as i64,
            is_private: domain.is_private,
            created_at: Some(to_timestamp(domain.created_at)),
            updated_at: Some(to_timestamp(domain.updated_at)),
            version: domain.metadata.version as i64,
        }
    }
}

impl From<SocialLinks> for ProtoSocialLinks {
    fn from(domain: SocialLinks) -> Self {
        Self {
            // Mapping direct des Value Objects (Url -> String -> StringValue)
            website_url: domain.website.map(|u| u.to_string().into()),
            linkedin_url: domain.linkedin.map(|u| u.to_string().into()),
            github_url: domain.github.map(|u| u.to_string().into()),
            x_url: domain.x.map(|u| u.to_string().into()),
            instagram_url: domain.instagram.map(|u| u.to_string().into()),
            facebook_url: domain.facebook.map(|u| u.to_string().into()),
            tiktok_url: domain.tiktok.map(|u| u.to_string().into()),
            youtube_url: domain.youtube.map(|u| u.to_string().into()),
            twitch_url: domain.twitch.map(|u| u.to_string().into()),
            discord_url: domain.discord.map(|u| u.to_string().into()),
            onlyfans_url: domain.onlyfans.map(|u| u.to_string().into()),

            // Conversion de la HashMap<String, Url> en HashMap<String, String>
            others: domain.others
                .into_iter()
                .map(|(k, v)| (k, v.to_string()))
                .collect(),
        }
    }
}

impl From<ProfileStats> for ProtoProfileStats {
    fn from(domain: ProfileStats) -> Self {
        Self {
            follower_count: domain.follower_count.value() as i64,
            following_count: domain.following_count.value() as i64,
        }
    }
}

impl From<Profile> for ProtoProfileSummary {
    fn from(domain: Profile) -> Self {
        Self {
            account_id: domain.account_id.to_string(),
            username: domain.username.to_string(),
            display_name: domain.display_name.to_string(),
            avatar_url: domain.avatar_url.map(|u| u.to_string()).unwrap_or_default(),
        }
    }
}