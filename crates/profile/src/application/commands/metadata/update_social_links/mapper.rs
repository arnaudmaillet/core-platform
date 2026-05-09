// crates/profile/src/application/commands/metadata/update_social_links/mapper.rs

use crate::domain::value_objects::SocialLinks;
use shared_kernel::{domain::value_objects::Url, errors::Result};
use shared_proto::profile::v1::SocialLinks as ProtoSocialLinks;

pub fn from_proto_to_social_links(proto: ProtoSocialLinks) -> Result<Option<SocialLinks>> {
    let mut links = SocialLinks::new();

    let to_url = |v: Option<String>| -> Result<Option<Url>> {
        v.filter(|s| !s.trim().is_empty())
            .map(Url::try_new)
            .transpose()
    };

    links = links
        .with_website(to_url(proto.website_url)?)
        .with_linkedin(to_url(proto.linkedin_url)?)
        .with_github(to_url(proto.github_url)?)
        .with_x(to_url(proto.x_url)?)
        .with_instagram(to_url(proto.instagram_url)?)
        .with_facebook(to_url(proto.facebook_url)?)
        .with_tiktok(to_url(proto.tiktok_url)?)
        .with_youtube(to_url(proto.youtube_url)?)
        .with_twitch(to_url(proto.twitch_url)?)
        .with_discord(to_url(proto.discord_url)?)
        .with_onlyfans(to_url(proto.onlyfans_url)?);

    for (key, val) in proto.others {
        if let Some(url) = to_url(Some(val))? {
            links = links.with_other(key, url);
        }
    }

    // On utilise la logique de validation et de vacuité du domaine
    links.try_build()
}
