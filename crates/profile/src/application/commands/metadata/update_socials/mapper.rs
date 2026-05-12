// crates/profile/src/application/commands/metadata/update_social_links/mapper.rs

use crate::value_objects::Socials;
use shared_kernel::{domain::value_objects::Url, core::Result};
use shared_proto::profile::v1::Socials as ProtoSocials;

pub fn from_proto_to_social_links(proto: ProtoSocials) -> Result<Option<Socials>> {
    let mut links = Socials::builder();

    let to_url = |v: Option<String>| -> Result<Option<Url>> {
        v.filter(|s| !s.trim().is_empty())
            .map(Url::try_new)
            .transpose()
    };

    if let Some(url) = to_url(proto.website_url)? {
        links = links.with_website(url);
    }
    if let Some(url) = to_url(proto.linkedin_url)? {
        links = links.with_linkedin(url);
    }
    if let Some(url) = to_url(proto.github_url)? {
        links = links.with_github(url);
    }
    if let Some(url) = to_url(proto.x_url)? {
        links = links.with_x(url);
    }
    if let Some(url) = to_url(proto.instagram_url)? {
        links = links.with_instagram(url);
    }
    if let Some(url) = to_url(proto.facebook_url)? {
        links = links.with_facebook(url);
    }
    if let Some(url) = to_url(proto.tiktok_url)? {
        links = links.with_tiktok(url);
    }
    if let Some(url) = to_url(proto.youtube_url)? {
        links = links.with_youtube(url);
    }
    if let Some(url) = to_url(proto.twitch_url)? {
        links = links.with_twitch(url);
    }
    if let Some(url) = to_url(proto.discord_url)? {
        links = links.with_discord(url);
    }
    if let Some(url) = to_url(proto.onlyfans_url)? {
        links = links.with_onlyfans(url);
    }

    for (key, val) in proto.others {
        if let Some(url) = to_url(Some(val))? {
            links = links.with_other(key, url);
        }
    }

    // On utilise la logique de validation et de vacuité du domaine
    links.try_build()
}
