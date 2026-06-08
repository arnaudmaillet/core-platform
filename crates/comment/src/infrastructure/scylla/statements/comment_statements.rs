// crates/content_comments/src/infrastructure/repositories/statements.rs

pub const INSERT_ROOT: &str = "
    INSERT INTO content_comments.comments_by_post (post_id, comment_id, profile_id, content, edited_at, updated_at) 
    VALUES (?, ?, ?, ?, ?, ?);
";

pub const INSERT_REPLY: &str = "
    INSERT INTO content_comments.replies_by_comment (parent_comment_id, comment_id, post_id, profile_id, content, edited_at, updated_at) 
    VALUES (?, ?, ?, ?, ?, ?, ?);
";

pub const FIND_ROOT_BY_ID: &str = "
    SELECT post_id, comment_id, profile_id, content, edited_at, updated_at 
    FROM content_comments.comments_by_post 
    WHERE post_id = ? AND comment_id = ? LIMIT 1;
";

pub const FIND_REPLY_BY_ID: &str = "
    SELECT parent_comment_id, comment_id, post_id, profile_id, content, edited_at, updated_at 
    FROM content_comments.replies_by_comment 
    WHERE parent_comment_id = ? AND comment_id = ? LIMIT 1;
";

pub const SELECT_ROOTS_BY_POST: &str = "
    SELECT post_id, comment_id, profile_id, content, edited_at, updated_at 
    FROM content_comments.comments_by_post 
    WHERE post_id = ?;
";

pub const SELECT_REPLIES_BY_PARENT: &str = "
    SELECT parent_comment_id, comment_id, post_id, profile_id, content, edited_at, updated_at 
    FROM content_comments.replies_by_comment 
    WHERE parent_comment_id = ?;
";

pub const DELETE_ROOT: &str =
    "DELETE FROM content_comments.comments_by_post WHERE post_id = ? AND comment_id = ?;";
pub const DELETE_REPLY: &str = "DELETE FROM content_comments.replies_by_comment WHERE parent_comment_id = ? AND comment_id = ?;";
