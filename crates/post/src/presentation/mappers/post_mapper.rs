// crates/post/src/presentation/mappers/mod.rs

use crate::entities::Post;
use shared_proto::post::v1::Post as ProtoPost;

impl Post {
    pub fn to_proto(&self) -> ProtoPost {
        let proto_updated_at = self.updated_at().map(|dt| prost_types::Timestamp {
            seconds: dt.timestamp(),
            nanos: dt.timestamp_subsec_nanos() as i32,
        });

        ProtoPost {
            post_id: self.post_id().to_string(),
            author_id: self.author_id().to_string(),
            post_type: self.post_type().to_string(),
            caption: self.caption().as_ref().map(|c| c.to_string()),
            media_list: self
                .media_list()
                .iter()
                .map(|m| shared_proto::post::v1::MediaAsset {
                    media_id: m.media_id().to_string(),
                    url: m.url().to_string(),
                    thumbnail_url: m.thumbnail_url().to_string(),
                    duration_seconds: m.duration_seconds().into(),
                    width: m.width().into(),
                    height: m.height().into(),
                    media_type: m.media_type().to_string(),
                    mime_type: m.mime_type().to_string(),
                })
                .collect(),
            total_duration_seconds: self.total_duration_seconds(),
            allowed_comment_hands: self.allowed_comment_hands(),
            visibility_level: self.visibility_level().to_string(),
            music_id: self.music_id().map(|id| id.to_string()),
            hashtags: self.hashtags().value().iter().cloned().collect(),
            is_edited: self.is_edited(),
            updated_at: proto_updated_at,
            dynamic_metadata: self.dynamic_metadata().to_string(),
        }
    }
}
