use std::collections::HashMap;
use shared_kernel::domain::value_objects::Url;
use crate::domain::value_objects::SocialLinks;
use super::super::profile_v1::SocialLinks as ProtoSocialLinks;

impl From<ProtoSocialLinks> for SocialLinks {
    fn from(proto: ProtoSocialLinks) -> Self {
        let mut others = HashMap::new();
        for (k, v) in proto.others {
            if let Ok(url) = Url::try_from(v) {
                others.insert(k, url);
            }
        }

        Self {
            website: proto.website_url.and_then(|u| Url::try_from(u).ok()),
            linkedin: proto.linkedin_url.and_then(|u| Url::try_from(u).ok()),
            github: proto.github_url.and_then(|u| Url::try_from(u).ok()),
            x: proto.x_url.and_then(|u| Url::try_from(u).ok()),
            instagram: proto.instagram_url.and_then(|u| Url::try_from(u).ok()),
            facebook: proto.facebook_url.and_then(|u| Url::try_from(u).ok()),
            tiktok: proto.tiktok_url.and_then(|u| Url::try_from(u).ok()),
            youtube: proto.youtube_url.and_then(|u| Url::try_from(u).ok()),
            twitch: proto.twitch_url.and_then(|u| Url::try_from(u).ok()),
            discord: proto.discord_url.and_then(|u| Url::try_from(u).ok()),
            onlyfans: proto.onlyfans_url.and_then(|u| Url::try_from(u).ok()),
            others,
        }
    }
}

impl From<SocialLinks> for ProtoSocialLinks{
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