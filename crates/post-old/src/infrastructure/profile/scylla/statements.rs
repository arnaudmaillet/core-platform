// crates/post/src/infrastructure/profile/scylla/statements.rs

pub const SAVE_PROFILE_PROJECTION: &str = r#"
    INSERT INTO {ks}.replicated_profiles_v1 (
        profile_id, handle, display_name, avatar_url, is_verified, updated_at
    ) VALUES (?, ?, ?, ?, ?, ?) USING TIMESTAMP ?
"#;

pub const FIND_PROFILE_PROJECTION_BY_ID: &str = r#"
    SELECT profile_id, handle, display_name, avatar_url, is_verified 
    FROM {ks}.replicated_profiles_v1 
    WHERE profile_id = ?
"#;
