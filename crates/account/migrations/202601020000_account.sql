-- 1. ENUMS
DO $$ BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'account_state') THEN
CREATE TYPE account_state AS ENUM ('pending', 'active', 'deactivated', 'suspended');
END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'internal_role') THEN
CREATE TYPE internal_role AS ENUM ('user', 'moderator', 'staff', 'admin');
END IF;
END $$;

-- 2. CORE ACCOUNT
CREATE TABLE IF NOT EXISTS accounts (
                                     id UUID PRIMARY KEY,
                                     region_code VARCHAR(10) NOT NULL DEFAULT 'eu',
    external_id TEXT NOT NULL UNIQUE,
    username TEXT NOT NULL UNIQUE,
    email TEXT UNIQUE,
    phone_number TEXT UNIQUE,
    email_verified BOOLEAN NOT NULL DEFAULT FALSE,
    account_state account_state NOT NULL DEFAULT 'active',
    version INT NOT NULL DEFAULT 1, -- Optimistic Locking
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
    );

-- 3. SETTINGS
CREATE TABLE IF NOT EXISTS user_settings (
                                             account_id UUID NOT NULL,
                                             region_code VARCHAR(10) NOT NULL DEFAULT 'eu',
    settings JSONB NOT NULL DEFAULT '{}',
    timezone TEXT NOT NULL DEFAULT 'UTC',
    push_tokens TEXT[] DEFAULT '{}',
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (account_id, region_code)
    );

-- 4. INTERNAL METADATA (Security & Trust)
CREATE TABLE IF NOT EXISTS user_internal_metadata (
                                                      account_id UUID NOT NULL,
                                                      region_code VARCHAR(10) NOT NULL DEFAULT 'eu',
    role internal_role NOT NULL DEFAULT 'user',
    is_beta_tester BOOLEAN NOT NULL DEFAULT FALSE,
    is_shadowbanned BOOLEAN NOT NULL DEFAULT FALSE,
    trust_score INT NOT NULL DEFAULT 1,
    moderation_notes TEXT,
    estimated_ip TEXT,
    version INT NOT NULL DEFAULT 1, -- Crucial pour l'idempotence du Trust Score
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (account_id, region_code)
    );

-- 5. INDEXES & TRIGGERS
CREATE INDEX IF NOT EXISTS idx_users_username_lower ON accounts (LOWER(username));
CREATE INDEX IF NOT EXISTS idx_user_settings_push_tokens ON user_settings USING GIN (push_tokens);

CREATE TRIGGER trg_set_timestamp_users BEFORE UPDATE ON users FOR EACH ROW EXECUTE PROCEDURE trigger_set_timestamp();
CREATE TRIGGER trg_set_timestamp_settings BEFORE UPDATE ON user_settings FOR EACH ROW EXECUTE PROCEDURE trigger_set_timestamp();
CREATE TRIGGER trg_set_timestamp_internal BEFORE UPDATE ON user_internal_metadata FOR EACH ROW EXECUTE PROCEDURE trigger_set_timestamp();