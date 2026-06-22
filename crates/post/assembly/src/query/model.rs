// crates/post/assembly/src/read_model.rs

use post_older::Post;
use post_profile::ProjectedProfile;

pub struct PostDetail {
    pub post: Post,
    pub author: ProjectedProfile,
}
