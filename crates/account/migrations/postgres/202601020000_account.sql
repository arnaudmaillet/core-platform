CREATE OR REPLACE FUNCTION trigger_set_timestamp()
RETURNS TRIGGER AS $$
BEGIN
  NEW.updated_at = NOW();
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;


-- 2. IDENTITY (Table racine)
CREATE TABLE IF NOT EXISTS account_identity (
    account_id UUID,
    region VARCHAR(10) NOT NULL,
    sub_id TEXT,
    email TEXT,
    phone_number TEXT,
    state TEXT NOT NULL DEFAULT 'PENDING',
    birth_date DATE,
    locale VARCHAR(10) NOT NULL DEFAULT 'EN',
    version BIGINT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    aggregate_updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_active_at TIMESTAMPTZ,
    PRIMARY KEY (region, account_id),
    CONSTRAINT uq_email UNIQUE (email, region),
    CONSTRAINT uq_phone_number UNIQUE (phone_number, region),
    CONSTRAINT uq_sub_id UNIQUE (sub_id)
);

-- 3. SETTINGS (Relation 1:1 co-localisée)
CREATE TABLE IF NOT EXISTS account_settings (
    account_id UUID,
    region VARCHAR(10) NOT NULL,
    preferences JSONB NOT NULL DEFAULT '{}',
    timezone TEXT NOT NULL DEFAULT 'UTC',
    push_tokens TEXT[] DEFAULT '{}',
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (region, account_id),
    CONSTRAINT fk_settings_identity FOREIGN KEY (region, account_id) REFERENCES account_identity(region, account_id) ON DELETE CASCADE
);

-- 4. GOVERNANCE (Relation 1:1 co-localisée)
CREATE TABLE IF NOT EXISTS account_governance (
    account_id UUID,
    region VARCHAR(10) NOT NULL,
    role TEXT NOT NULL DEFAULT 'USER',
    beta_tier TEXT NOT NULL DEFAULT 'NONE',
    is_shadowbanned BOOLEAN NOT NULL DEFAULT FALSE,
    trust_score INT NOT NULL DEFAULT 100,
    moderation_notes TEXT,
    last_moderation_at TIMESTAMPTZ,
    last_ip_addr INET,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (region, account_id),
    CONSTRAINT fk_governance_identity FOREIGN KEY (region, account_id) REFERENCES account_identity(region, account_id) ON DELETE CASCADE
);

-- 5. INDEXATION
CREATE INDEX IF NOT EXISTS idx_accounts_sub_id ON account_identity (region, sub_id);
CREATE INDEX IF NOT EXISTS idx_governance_flagged ON account_governance (region, account_id) 
WHERE is_shadowbanned IS TRUE OR trust_score < 50;

-- 6. TRIGGERS (Automatisation du updated_at)
DROP TRIGGER IF EXISTS trg_set_timestamp_identity ON account_identity;
CREATE TRIGGER trg_set_timestamp_identity BEFORE UPDATE ON account_identity FOR EACH ROW EXECUTE PROCEDURE trigger_set_timestamp();

DROP TRIGGER IF EXISTS trg_set_timestamp_settings ON account_settings;
CREATE TRIGGER trg_set_timestamp_settings BEFORE UPDATE ON account_settings FOR EACH ROW EXECUTE PROCEDURE trigger_set_timestamp();

DROP TRIGGER IF EXISTS trg_set_timestamp_governance ON account_governance;
CREATE TRIGGER trg_set_timestamp_governance BEFORE UPDATE ON account_governance FOR EACH ROW EXECUTE PROCEDURE trigger_set_timestamp();