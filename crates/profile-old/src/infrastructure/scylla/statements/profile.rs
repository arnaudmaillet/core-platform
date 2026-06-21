// crates/profile/src/infrastructure/scylla/statements/profile_statement.rs

pub const INSERT_PROFILE: &str = r#"
    INSERT INTO {ks}.profiles (
        id, account_id, handle, display_name, bio, avatar_url, banner_url, location_label, social_links, is_private, version, created_at, updated_at
    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) IF NOT EXISTS
"#;

pub const UPDATE_PROFILE: &str = r#"
    UPDATE {ks}.profiles 
    SET display_name = ?, handle = ?, bio = ?, avatar_url = ?, banner_url = ?, location_label = ?, social_links = ?, is_private = ?, version = ?, updated_at = ? 
    WHERE id = ? 
    IF version = ?
"#;

pub const INSERT_PROFILE_BY_ACCOUNT: &str = r#"
    INSERT INTO {ks}.profiles_by_account (
        account_id, profile_id, handle, display_name, avatar_url, is_private
    ) VALUES (?, ?, ?, ?, ?, ?)
"#;

pub const DELETE_PROFILE_BY_ACCOUNT: &str = r#"
    DELETE FROM {ks}.profiles_by_account WHERE account_id = ? AND profile_id = ?
"#;

pub const FIND_BY_ID: &str = "SELECT * FROM {ks}.profiles WHERE id = ?";

pub const FIND_ALL_BY_ACCOUNT_ID: &str =
    "SELECT * FROM {ks}.profiles_by_account WHERE account_id = ?";

pub const DELETE_PROFILE: &str = "DELETE FROM {ks}.profiles WHERE id = ?";
