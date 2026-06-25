-- Migration: 0001_create_auth_tables
-- Description: Durable state for the auth bounded context — session ledger,
--              refresh-token rotation lineage, the immutable IdP-subject ↔ account
--              link, and the edge-token signing-key ring. Pure ANSI SQL, portable
--              across PostgreSQL and CockroachDB. All tables carry an account_id so
--              a person's auth state co-locates on one shard (ShardKey = account_id).

-- ── Sessions ─────────────────────────────────────────────────────────────────
-- One row per authentication act. `generation` is the epoch the session was
-- minted under; a global sign-out bumps the account's generation in Redis and
-- the durable counter, invalidating every session below it at the edge.
CREATE TABLE IF NOT EXISTS sessions (
    id                  UUID         NOT NULL,
    account_id          UUID         NOT NULL,
    issuer              TEXT         NOT NULL,
    subject             TEXT         NOT NULL,
    generation          BIGINT       NOT NULL,
    status              TEXT         NOT NULL DEFAULT 'active',
    device_user_agent   TEXT,
    device_ip           TEXT,
    device_id           TEXT,
    issued_at           TIMESTAMPTZ  NOT NULL,
    expires_at          TIMESTAMPTZ  NOT NULL,
    absolute_expiry     TIMESTAMPTZ  NOT NULL,
    revoked_at          TIMESTAMPTZ,
    version             BIGINT       NOT NULL DEFAULT 0,
    PRIMARY KEY (id)
);

-- Device-management view + the set a global sign-out iterates.
CREATE INDEX IF NOT EXISTS idx_sessions_account_active
    ON sessions (account_id, status);

-- ── Refresh tokens ───────────────────────────────────────────────────────────
-- Opaque, single-use, rotated on every exchange. Only the hash is stored;
-- `replaced_by` chains the rotation lineage for reuse-detection.
CREATE TABLE IF NOT EXISTS refresh_tokens (
    id            UUID         NOT NULL,
    session_id    UUID         NOT NULL,
    account_id    UUID         NOT NULL,
    token_hash    TEXT         NOT NULL,
    status        TEXT         NOT NULL DEFAULT 'active',
    issued_at     TIMESTAMPTZ  NOT NULL,
    expires_at    TIMESTAMPTZ  NOT NULL,
    used_at       TIMESTAMPTZ,
    replaced_by   UUID,
    version       BIGINT       NOT NULL DEFAULT 0,
    PRIMARY KEY (id),
    CONSTRAINT uq_refresh_tokens_hash UNIQUE (token_hash)
);

CREATE INDEX IF NOT EXISTS idx_refresh_tokens_session
    ON refresh_tokens (session_id);

-- ── Subject links ────────────────────────────────────────────────────────────
-- The immutable (issuer, subject) → account_id binding. Keyed on the full pair so
-- an IdP migration (new issuer) never collides with existing links.
CREATE TABLE IF NOT EXISTS subject_links (
    issuer       TEXT         NOT NULL,
    subject      TEXT         NOT NULL,
    account_id   UUID         NOT NULL,
    linked_at    TIMESTAMPTZ  NOT NULL,
    version      BIGINT       NOT NULL DEFAULT 0,
    PRIMARY KEY (issuer, subject)
);

CREATE INDEX IF NOT EXISTS idx_subject_links_account
    ON subject_links (account_id);

-- ── Signing keys ─────────────────────────────────────────────────────────────
-- The edge-token signing-key ring. Phase 4's minter loads its key from config;
-- this table is the home for in-DB rotation (publish public material via JWKS,
-- keep the private key encrypted at rest / in KMS). No adapter writes it yet.
CREATE TABLE IF NOT EXISTS signing_keys (
    kid                 TEXT         NOT NULL,
    algorithm           TEXT         NOT NULL DEFAULT 'ES256',
    public_pem          TEXT         NOT NULL,
    private_pem_enc     BYTEA,
    status              TEXT         NOT NULL DEFAULT 'active',
    created_at          TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    not_after           TIMESTAMPTZ,
    PRIMARY KEY (kid)
);
