use post::{Media, Post};
use post_assembly::PostDetail;
use post_proto_bridge::v1::{
    MediaAsset as ProtoMediaAsset, Post as ProtoPost, PostDetails as ProtoPostDetails,
};
use shared_kernel::core::{ManagedEntity, Versioned};

pub struct GrpcPostQueryMapper;

impl GrpcPostQueryMapper {
    pub fn to_proto_details(hydrated: &PostDetail) -> ProtoPostDetails {
        ProtoPostDetails {
            post: Some(Self::to_proto_raw(&hydrated.post)),
            author_display_name: hydrated.author.display_name.clone(),
            author_avatar_url: hydrated.author.avatar_url.clone().unwrap_or_default(),
        }
    }

    fn to_proto_raw(post: &Post) -> ProtoPost {
        let proto_edited_at = post.edited_at().map(|dt| prost_types::Timestamp {
            seconds: dt.timestamp(),
            nanos: dt.timestamp_subsec_nanos() as i32,
        });

        ProtoPost {
            post_id: post.post_id().to_string(),
            author_id: post.author_id().to_string(),
            post_type: post.post_type().to_string(),
            caption: post.caption().as_ref().map(|c| c.to_string()),
            media_list: post.media_list().iter().map(Self::media_to_proto).collect(),
            total_duration_seconds: post.total_duration_seconds(),
            allowed_comment_hands: post.allowed_comment_hands(),
            visibility_level: post.visibility_level().to_string(),
            music_id: post.music_id().map(|id| id.to_string()),
            hashtags: post.hashtags().iter().cloned().collect(),
            edited_at: proto_edited_at,
            dynamic_metadata: post.dynamic_metadata().to_string(),
            version: post.version(),
            created_at: Some(prost_types::Timestamp {
                seconds: post.created_at().timestamp(),
                nanos: post.created_at().timestamp_subsec_nanos() as i32,
            }),
            updated_at: Some(prost_types::Timestamp {
                seconds: post.lifecycle().updated_at().timestamp(),
                nanos: post.lifecycle().updated_at().timestamp_subsec_nanos() as i32,
            }),
        }
    }

    fn media_to_proto(media: &Media) -> ProtoMediaAsset {
        ProtoMediaAsset {
            media_id: media.media_id().to_string(),
            url: media.url().to_string(),
            thumbnail_url: media.thumbnail_url().to_string(),
            duration_seconds: media.duration_seconds().value(),
            width: media.width().value(),
            height: media.height().value(),
            media_type: media.media_type().to_string(),
            mime_type: media.mime_type().to_string(),
        }
    }
}
