use super::super::profile_v1::SocialLinks as ProtoSocialLinks;
use crate::domain::value_objects::SocialLinks;
use shared_kernel::domain::value_objects::Url;
use std::collections::HashMap;
use shared_kernel::errors::DomainError;

impl TryFrom<ProtoSocialLinks> for SocialLinks {
    type Error = DomainError;

    fn try_from(proto: ProtoSocialLinks) -> Result<Self, Self::Error> {
        let mut builder = SocialLinks::new()
            .with_website(proto.website_url.and_then(|u| Url::try_from(u).ok()))
            .with_linkedin(proto.linkedin_url.and_then(|u| Url::try_from(u).ok()))
            .with_github(proto.github_url.and_then(|u| Url::try_from(u).ok()))
            .with_x(proto.x_url.and_then(|u| Url::try_from(u).ok()))
            .with_instagram(proto.instagram_url.and_then(|u| Url::try_from(u).ok()))
            .with_facebook(proto.facebook_url.and_then(|u| Url::try_from(u).ok()))
            .with_tiktok(proto.tiktok_url.and_then(|u| Url::try_from(u).ok()))
            .with_youtube(proto.youtube_url.and_then(|u| Url::try_from(u).ok()))
            .with_twitch(proto.twitch_url.and_then(|u| Url::try_from(u).ok()))
            .with_discord(proto.discord_url.and_then(|u| Url::try_from(u).ok()))
            .with_onlyfans(proto.onlyfans_url.and_then(|u| Url::try_from(u).ok()));

        for (k, v) in proto.others {
            if let Ok(url) = Url::try_from(v) {
                builder = builder.with_other(k, url);
            }
        }

        builder.try_build()?.ok_or(DomainError::Validation {
            field: "social_links",
            reason: "At least one social link must be valid".into(),
        })
    }
}

impl From<SocialLinks> for ProtoSocialLinks {
    fn from(domain: SocialLinks) -> Self {
        Self {
            website_url: domain.website().map(|u| u.to_string()),
            linkedin_url: domain.linkedin().map(|u| u.to_string()),
            github_url: domain.github().map(|u| u.to_string()),
            x_url: domain.x().map(|u| u.to_string()),
            instagram_url: domain.instagram().map(|u| u.to_string()),
            facebook_url: domain.facebook().map(|u| u.to_string()),
            tiktok_url: domain.tiktok().map(|u| u.to_string()),
            youtube_url: domain.youtube().map(|u| u.to_string()),
            twitch_url: domain.twitch().map(|u| u.to_string()),
            discord_url: domain.discord().map(|u| u.to_string()),
            onlyfans_url: domain.onlyfans().map(|u| u.to_string()),
            others: domain
                .others()
                .iter()
                .map(|(k, v)| (k.clone(), v.to_string()))
                .collect(),
        }
    }
}