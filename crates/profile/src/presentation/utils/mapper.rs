// crates/profile/src/presentation/utils/mapper.rs

use crate::entities::Profile;
use crate::value_objects::Socials;
use shared_kernel::domain::{entities::Versioned, events::AggregateRoot};
use shared_proto::profile::v1::{Profile as ProfileProto, Socials as SocialsProto};

pub fn map_profile_to_proto(profile: Profile) -> ProfileProto {
    ProfileProto {
        profile_id: profile.id().to_string(),
        account_id: profile.account_id().to_string(),
        handle: profile.handle().to_string(),
        display_name: profile.display_name().to_string(),
        is_private: profile.is_private(),
        bio: profile.bio().map(|b| b.to_string()),
        location: profile.location().map(|l| l.to_string()),
        avatar_url: profile.avatar().map(|u| u.to_string()),
        banner_url: profile.banner().map(|u| u.to_string()),
        socials: profile.socials().map(map_social_links_to_proto),
        version: profile.version(),
        region_code: profile.account_id().region().to_string(),
        created_at: Some(to_proto_timestamp(profile.created_at())),
        updated_at: Some(to_proto_timestamp(profile.updated_at())),
    }
}

fn to_proto_timestamp(dt: chrono::DateTime<chrono::Utc>) -> prost_types::Timestamp {
    prost_types::Timestamp {
        seconds: dt.timestamp(),
        nanos: dt.timestamp_subsec_nanos() as i32,
    }
}

fn map_social_links_to_proto(links: &Socials) -> SocialsProto {
    SocialsProto {
        website_url: links.website().map(|u| u.to_string()),
        linkedin_url: links.linkedin().map(|u| u.to_string()),
        github_url: links.github().map(|u| u.to_string()),
        x_url: links.x().map(|u| u.to_string()),
        instagram_url: links.instagram().map(|u| u.to_string()),
        facebook_url: links.facebook().map(|u| u.to_string()),
        tiktok_url: links.tiktok().map(|u| u.to_string()),
        youtube_url: links.youtube().map(|u| u.to_string()),
        twitch_url: links.twitch().map(|u| u.to_string()),
        discord_url: links.discord().map(|u| u.to_string()),
        onlyfans_url: links.onlyfans().map(|u| u.to_string()),

        // Conversion de HashMap<String, Url> en HashMap<String, String>
        others: links
            .others()
            .iter()
            .map(|(k, v)| (k.clone(), v.to_string()))
            .collect(),
    }
}
