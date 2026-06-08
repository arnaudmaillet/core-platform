// crates/content_comments/src/presentation/mappers/mod.rs

use shared_proto::comment::v1::Comment as ProtoComment;
use shared_proto::comment::v1::CommentAuthor as ProtoCommentAuthor;

use crate::application::dtos::CommentWithProfile;

impl CommentWithProfile {
    pub fn to_proto(&self) -> ProtoComment {
        let proto_created_at = Some(prost_types::Timestamp {
            seconds: self.comment.created_at().timestamp(),
            nanos: self.comment.created_at().timestamp_subsec_nanos() as i32,
        });

        let proto_edited_at = self.comment.edited_at().map(|dt| prost_types::Timestamp {
            seconds: dt.timestamp(),
            nanos: dt.timestamp_subsec_nanos() as i32,
        });

        let proto_author = self
            .profile
            .as_ref()
            .map(|p| ProtoCommentAuthor {
                profile_id: p.profile_id().to_string(),
                username: p.username().to_string(),
                display_name: p.display_name().to_string(),
                avatar_url: p.avatar_url().unwrap_or("").to_string(),
            })
            .unwrap_or_default();

        // 3. Assemblage du payload gRPC final
        ProtoComment {
            comment_id: self.comment.comment_id().to_string(),
            post_id: self.comment.post_id().to_string(),
            profile_id: self.comment.profile_id().to_string(),
            parent_comment_id: self.comment.parent_comment_id().map(|id| id.to_string()),
            content: self.comment.content().to_string(),
            created_at: proto_created_at,
            edited_at: proto_edited_at,
            author: Some(proto_author),
        }
    }
}
