-- crates/shared-kernel/migrations/postgres/202601010000_foundation.sql

-- 0. EXTENSIONS
CREATE EXTENSION IF NOT EXISTS postgis SCHEMA public;

-- 1. UTILITIES
CREATE OR REPLACE FUNCTION public.trigger_set_timestamp()
    RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- 2. TRANSACTIONAL OUTBOX
-- Table centrale pour la livraison garantie des messages
CREATE TABLE IF NOT EXISTS outbox_events (
                                             id UUID,
                                             region_code VARCHAR(10) NOT NULL,
                                             aggregate_type TEXT NOT NULL,
                                             aggregate_id TEXT NOT NULL,
                                             event_type TEXT NOT NULL,
                                             payload JSONB NOT NULL,
                                             metadata JSONB,
                                             occurred_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                                             processed_at TIMESTAMPTZ,
                                             PRIMARY KEY (id, region_code)
);

CREATE INDEX IF NOT EXISTS idx_outbox_unprocessed
    ON outbox_events (occurred_at) WHERE processed_at IS NULL;