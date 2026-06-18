// crates/post/src/infrastructure/post/scylla/statements.rs

pub const INSERT_POST_BY_AUTHOR: &str = r#"
    INSERT INTO {ks}.posts_by_author (
        author_id, post_id, post_type, caption, media_list, total_duration_seconds, 
        allowed_comment_hands, visibility_level, music_id, hashtags, mentions, 
        version, edited_at, created_at, updated_at, dynamic_metadata
    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
"#;

pub const INSERT_POST_BY_ID: &str = r#"
    INSERT INTO {ks}.posts_by_id (
        post_id, author_id, post_type, caption, media_list, total_duration_seconds, 
        allowed_comment_hands, visibility_level, music_id, hashtags, mentions, 
        version, edited_at, created_at, updated_at, dynamic_metadata
    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
"#;

pub const FIND_POST_BY_ID: &str = r#"
    SELECT * FROM {ks}.posts_by_id WHERE post_id = ? LIMIT 1
"#;

pub const FIND_POSTS_BY_AUTHOR: &str = r#"
    SELECT * FROM {ks}.posts_by_author WHERE author_id = ?
"#;

pub const DELETE_POST_BY_AUTHOR: &str = r#"
    DELETE FROM {ks}.posts_by_author WHERE author_id = ? AND post_id = ?
"#;

pub const DELETE_POST_BY_ID: &str = r#"
    DELETE FROM {ks}.posts_by_id WHERE post_id = ?
"#;
