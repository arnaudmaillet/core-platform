// crates/content_comments/src/infrastructure/scylla/statements.rs

pub const INSERT_COMMENT: &str = "
    INSERT INTO content_comments.comments (comment_id, post_id, profile_id, parent_comment_id, content, created_at, edited_at)
    VALUES (?, ?, ?, ?, ?, ?, ?);
";

pub const INSERT_PROFILE: &str = "
    INSERT INTO content_comments.comment_user_profiles (profile_id, username, display_name, avatar_url, updated_at)
    VALUES (?, ?, ?, ?, ?);
";

pub const FIND_PROFILES_BATCH: &str = "
    SELECT profile_id, username, display_name, avatar_url 
    FROM content_comments.comment_user_profiles 
    WHERE profile_id IN ?;
";
