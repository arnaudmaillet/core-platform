use crate::clients::profile as proto;
use async_graphql::{ID, SimpleObject};

#[derive(SimpleObject)]
pub struct Profile {
    pub account_id: ID,
    pub username: String,
    pub display_name: String,
    pub bio: Option<String>,
    pub avatar_url: Option<String>,
    pub banner_url: Option<String>,
    pub stats: ProfileStats,
    pub post_count: i64,
    pub is_private: bool,
}

#[derive(SimpleObject)]
pub struct ProfileStats {
    pub follower_count: i64,
    pub following_count: i64,
}

// Mapper pour transformer la réponse gRPC en objet GraphQL
impl From<proto::Profile> for Profile {
    fn from(p: proto::Profile) -> Self {
        Self {
            account_id: ID(p.account_id),
            username: p.username,
            display_name: p.display_name,
            bio: p.bio.map(|v| v.to_string()),
            avatar_url: p.avatar_url.map(|v| v.to_string()),
            banner_url: p.banner_url.map(|v| v.to_string()),
            post_count: p.post_count,
            is_private: p.is_private,
            stats: p
                .stats
                .map(|s| ProfileStats {
                    follower_count: s.follower_count,
                    following_count: s.following_count,
                })
                .unwrap_or_else(|| ProfileStats {
                    follower_count: 0,
                    following_count: 0,
                }),
        }
    }
}
