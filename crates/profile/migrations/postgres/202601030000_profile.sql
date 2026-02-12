-- crates/profile/migrations/postgres/202601030000_profile.sql

-- 0. SHARED LOGIC
CREATE OR REPLACE FUNCTION public.trigger_set_timestamp()
RETURNS TRIGGER AS $$
BEGIN
  NEW.updated_at = NOW();
RETURN NEW;
END;
$$ language 'plpgsql';

-- 1. USER PROFILES (Social View)
CREATE TABLE IF NOT EXISTS user_profiles (
    id UUID NOT NULL,
    owner_id UUID NOT NULL,
    region_code VARCHAR(10) NOT NULL,
    display_name VARCHAR(50) NOT NULL,
    handle VARCHAR(30) NOT NULL,
    bio VARCHAR(255),
    avatar_url TEXT,
    banner_url TEXT,
    location_label VARCHAR(100),
    social_links JSONB DEFAULT '{}'::jsonb NOT NULL,
    is_private BOOLEAN DEFAULT FALSE NOT NULL,
    post_count BIGINT DEFAULT 0 NOT NULL,
    version BIGINT NOT NULL DEFAULT 1,
    created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    updated_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,

    PRIMARY KEY (id, region_code),
    CONSTRAINT uq_user_profiles_handle UNIQUE (handle)
    );

-- 2. LOCATIONS (High Frequency Updates)
CREATE TABLE IF NOT EXISTS user_locations (
    profile_id UUID NOT NULL,
    region_code VARCHAR(10) NOT NULL,
    coordinates GEOGRAPHY(POINT, 4326) NOT NULL,
    accuracy_meters DOUBLE PRECISION,
    altitude DOUBLE PRECISION,
    heading DOUBLE PRECISION,
    speed DOUBLE PRECISION,
    is_ghost_mode BOOLEAN NOT NULL DEFAULT FALSE,
    privacy_radius_meters INT DEFAULT 0,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    version BIGINT NOT NULL DEFAULT 1,

    PRIMARY KEY (profile_id, region_code)
    );

-- 4. PERFORMANCE INDEXES
CREATE UNIQUE INDEX IF NOT EXISTS idx_user_profiles_profile_global ON user_profiles (handle);
CREATE INDEX IF NOT EXISTS idx_user_locations_gist ON user_locations USING GIST (coordinates);

-- 5. TRIGGERS
DROP TRIGGER IF EXISTS trg_set_timestamp_profiles ON user_profiles;
CREATE TRIGGER trg_set_timestamp_profiles BEFORE UPDATE ON user_profiles FOR EACH ROW EXECUTE PROCEDURE public.trigger_set_timestamp();

DROP TRIGGER IF EXISTS trg_set_timestamp_locations ON user_locations;
CREATE TRIGGER trg_set_timestamp_locations BEFORE UPDATE ON user_locations FOR EACH ROW EXECUTE PROCEDURE public.trigger_set_timestamp();