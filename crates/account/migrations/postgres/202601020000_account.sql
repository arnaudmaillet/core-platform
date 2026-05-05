-- 1. ENUMS (Identité et rôles)
DO $$ BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'account_state') THEN
        CREATE TYPE account_state AS ENUM ('PENDING', 'ACTIVE', 'DEACTIVATED', 'SUSPENDED', 'BANNED');
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'internal_role') THEN
        CREATE TYPE internal_role AS ENUM ('USER', 'MODERATOR', 'STAFF', 'ADMIN');
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'beta_tier') THEN
        CREATE TYPE beta_tier AS ENUM ('NONE', 'BETA', 'ALPHA', 'INTERNAL');
END IF;
END $$;

-- 2. FONCTION DE TRIGGER (Crucial : à définir AVANT les tables)
CREATE OR REPLACE FUNCTION trigger_set_timestamp()
RETURNS TRIGGER AS $$
BEGIN
  NEW.updated_at = NOW();
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;


-- 3. IDENTITY (Table racine)
CREATE TABLE IF NOT EXISTS account_identity (
    account_id UUID PRIMARY KEY,
    sub_id TEXT,
    email TEXT UNIQUE,
    phone_number TEXT UNIQUE,
    state account_state NOT NULL DEFAULT 'PENDING',
    birth_date DATE,
    locale VARCHAR(10) NOT NULL DEFAULT 'en',
    region_code VARCHAR(10) NOT NULL,
    version BIGINT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    aggregate_updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_active_at TIMESTAMPTZ,
    CONSTRAINT uq_sub_id UNIQUE (sub_id)
);

-- 4. SETTINGS (Relation 1:1 co-localisée)
CREATE TABLE IF NOT EXISTS account_settings (
    account_id UUID PRIMARY KEY,
    preferences JSONB NOT NULL DEFAULT '{}',
    timezone TEXT NOT NULL DEFAULT 'UTC',
    push_tokens TEXT[] DEFAULT '{}',
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT fk_settings_identity FOREIGN KEY (account_id) REFERENCES account_identity(account_id) ON DELETE CASCADE
);

-- 5. GOVERNANCE (Relation 1:1 co-localisée)
CREATE TABLE IF NOT EXISTS account_governance (
    account_id UUID PRIMARY KEY,
    role internal_role NOT NULL DEFAULT 'USER',
    beta_tier beta_tier NOT NULL DEFAULT 'NONE',
    is_shadowbanned BOOLEAN NOT NULL DEFAULT FALSE,
    trust_score INT NOT NULL DEFAULT 100,
    moderation_notes TEXT,
    last_moderation_at TIMESTAMPTZ,
    last_ip_addr INET,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT fk_governance_identity FOREIGN KEY (account_id) REFERENCES account_identity(account_id) ON DELETE CASCADE
);

-- 6. INDEXATION
CREATE INDEX IF NOT EXISTS idx_accounts_sub_id ON account_identity (sub_id);
CREATE INDEX IF NOT EXISTS idx_governance_flagged ON account_governance (account_id) 
WHERE is_shadowbanned IS TRUE OR trust_score < 50;

-- 7. TRIGGERS (Automatisation du updated_at)
DROP TRIGGER IF EXISTS trg_set_timestamp_identity ON account_identity;
CREATE TRIGGER trg_set_timestamp_identity BEFORE UPDATE ON account_identity FOR EACH ROW EXECUTE PROCEDURE trigger_set_timestamp();

DROP TRIGGER IF EXISTS trg_set_timestamp_settings ON account_settings;
CREATE TRIGGER trg_set_timestamp_settings BEFORE UPDATE ON account_settings FOR EACH ROW EXECUTE PROCEDURE trigger_set_timestamp();

DROP TRIGGER IF EXISTS trg_set_timestamp_governance ON account_governance;
CREATE TRIGGER trg_set_timestamp_governance BEFORE UPDATE ON account_governance FOR EACH ROW EXECUTE PROCEDURE trigger_set_timestamp();

-- 7. INDEXATION (En dernier)
CREATE INDEX IF NOT EXISTS idx_accounts_sub_id ON account_identity (sub_id);