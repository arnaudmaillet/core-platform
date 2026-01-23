-- 1. USER PROFILES (Social View)
CREATE TABLE IF NOT EXISTS user_profiles (
                                             account_id UUID NOT NULL,
                                             region_code VARCHAR(10) NOT NULL,
    display_name VARCHAR(50) NOT NULL,
    username VARCHAR(30) NOT NULL,
    bio VARCHAR(255),
    avatar_url TEXT,
    banner_url TEXT,
    location_label VARCHAR(100),
    social_links JSONB DEFAULT '{}'::jsonb NOT NULL,
    is_private BOOLEAN DEFAULT FALSE NOT NULL,
    post_count BIGINT DEFAULT 0 NOT NULL,
    version INT NOT NULL DEFAULT 1,
    created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    updated_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    PRIMARY KEY (account_id, region_code)
    );

-- 2. LOCATIONS (High Frequency Updates)
CREATE TABLE IF NOT EXISTS user_locations (
                                              account_id UUID NOT NULL,
                                              region_code VARCHAR(10) NOT NULL,
    coordinates GEOGRAPHY(POINT, 4326) NOT NULL,
    is_ghost_mode BOOLEAN NOT NULL DEFAULT FALSE,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (account_id, region_code)
    );


-- 4. PERFORMANCE INDEXES
CREATE INDEX IF NOT EXISTS idx_user_locations_gist ON user_locations USING GIST (coordinates);
CREATE INDEX IF NOT EXISTS idx_user_profiles_username ON user_profiles (username);

-- 5. TRIGGERS
CREATE TRIGGER trg_set_timestamp_profiles BEFORE UPDATE ON user_profiles FOR EACH ROW EXECUTE PROCEDURE trigger_set_timestamp();
CREATE TRIGGER trg_set_timestamp_locations BEFORE UPDATE ON user_locations FOR EACH ROW EXECUTE PROCEDURE trigger_set_timestamp();