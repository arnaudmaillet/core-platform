// crates/post/assembly/src/read_model.rs

use post::Post;
use post_profile::ProjectedProfile;

pub struct PostDetail {
    pub post: Post,
    pub author: ProjectedProfile,
}
