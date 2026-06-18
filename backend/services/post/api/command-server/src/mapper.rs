use post_proto_bridge::v1::CreatePostResponse;
use shared_kernel::types::PostId;

pub struct GrpcPostCommandMapper;

impl GrpcPostCommandMapper {
    pub fn to_create_response(post_id: &PostId) -> CreatePostResponse {
        CreatePostResponse {
            post_id: post_id.to_string(),
        }
    }
}
