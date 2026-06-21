// crates/profile/src/infrastructure/scylla/statements/routing_statement.rs

pub const INSERT_ROUTING_PROFILE: &str = r#"
    INSERT INTO global_routing.profiles (profile_id, region) VALUES (?, ?)
"#;

pub const INSERT_ROUTING_SLUG: &str = r#"
    INSERT INTO global_routing.slugs (slug_hash, profile_id, region) VALUES (?, ?, ?) IF NOT EXISTS
"#;

pub const FIND_REGION_BY_ID: &str = r#"
    SELECT region FROM global_routing.profiles WHERE profile_id = ?
"#;

pub const FIND_ROUTING_BY_SLUG: &str = r#"
    SELECT profile_id, region FROM global_routing.slugs WHERE slug_hash = ?
"#;

pub const DELETE_ROUTING_PROFILE: &str = r#"
    DELETE FROM global_routing.profiles WHERE profile_id = ?
"#;

pub const DELETE_ROUTING_SLUG: &str = r#"
    DELETE FROM global_routing.slugs WHERE slug_hash = ?
"#;
