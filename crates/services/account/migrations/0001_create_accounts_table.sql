-- Migration: 0001_create_accounts_table
-- Description: Initial accounts table supporting both PostgreSQL and CockroachDB.
--              Uses flat columns for all domain fields; arrays for multi-value
--              attributes (roles, permissions, recovery codes). No stored
--              procedures; pure ANSI SQL with standard TIMESTAMPTZ defaults.
--              Financial state is managed by the dedicated ledger microservice.

CREATE TABLE IF NOT EXISTS accounts (
    -- ── Identity ─────────────────────────────────────────────────────────────
    id                                  UUID            NOT NULL,
    identity_id                         TEXT            NOT NULL,

    -- ── Lifecycle & status ────────────────────────────────────────────────────
    status                              TEXT            NOT NULL DEFAULT 'pending_verification',
    suspension_reason                   TEXT,
    deactivated_at                      TIMESTAMPTZ,

    -- ── Contact & verification ────────────────────────────────────────────────
    email                               TEXT            NOT NULL,
    email_verified                      BOOLEAN         NOT NULL DEFAULT FALSE,
    email_verified_at                   TIMESTAMPTZ,
    phone                               TEXT,
    phone_verified                      BOOLEAN         NOT NULL DEFAULT FALSE,
    phone_verified_at                   TIMESTAMPTZ,

    -- ── Credentials ──────────────────────────────────────────────────────────
    password_hash                       TEXT,
    password_changed_at                 TIMESTAMPTZ,
    failed_login_attempts               SMALLINT        NOT NULL DEFAULT 0,
    locked_until                        TIMESTAMPTZ,
    last_login_at                       TIMESTAMPTZ,

    -- ── MFA state ────────────────────────────────────────────────────────────
    mfa_enforced                        BOOLEAN         NOT NULL DEFAULT FALSE,
    mfa_totp_secret                     BYTEA,
    mfa_totp_enrolled_at                TIMESTAMPTZ,
    mfa_recovery_codes                  TEXT[]          NOT NULL DEFAULT '{}',
    mfa_backup_verified_at              TIMESTAMPTZ,

    -- ── KYC / identity verification ───────────────────────────────────────────
    kyc_status                          TEXT            NOT NULL DEFAULT 'not_started',
    kyc_reviewed_at                     TIMESTAMPTZ,
    kyc_reviewer_id                     UUID,
    date_of_birth                       DATE,
    country_of_residence                CHAR(2),

    -- ── GDPR / compliance ────────────────────────────────────────────────────
    gdpr_data_processing_consented_at   TIMESTAMPTZ,
    gdpr_marketing_consented_at         TIMESTAMPTZ,
    gdpr_consent_ip                     TEXT,
    gdpr_last_consent_version           TEXT,
    gdpr_deletion_requested_at          TIMESTAMPTZ,
    gdpr_deletion_scheduled_at          TIMESTAMPTZ,
    gdpr_anonymized_at                  TIMESTAMPTZ,
    gdpr_data_export_requested_at       TIMESTAMPTZ,
    gdpr_data_export_completed_at       TIMESTAMPTZ,

    -- ── Roles & permissions ───────────────────────────────────────────────────
    roles                               TEXT[]          NOT NULL DEFAULT '{}',
    permission_overrides                TEXT[]          NOT NULL DEFAULT '{}',

    -- ── Optimistic concurrency & audit ───────────────────────────────────────
    version                             BIGINT          NOT NULL DEFAULT 0,
    created_at                          TIMESTAMPTZ     NOT NULL DEFAULT NOW(),
    updated_at                          TIMESTAMPTZ     NOT NULL DEFAULT NOW(),
    created_by                          UUID,

    CONSTRAINT accounts_pkey PRIMARY KEY (id)
);

-- ── Indexes ───────────────────────────────────────────────────────────────────

-- Auth flow: look up account by IdP subject (most frequent read path).
CREATE UNIQUE INDEX IF NOT EXISTS accounts_identity_id_uidx
    ON accounts (identity_id);

-- Registration guard: prevent duplicate email registrations.
CREATE UNIQUE INDEX IF NOT EXISTS accounts_email_uidx
    ON accounts (email);

-- Admin / ops queries: list accounts by lifecycle status.
CREATE INDEX IF NOT EXISTS accounts_status_idx
    ON accounts (status);

-- Compliance queries: filter accounts by KYC outcome.
CREATE INDEX IF NOT EXISTS accounts_kyc_status_idx
    ON accounts (kyc_status);

-- GDPR janitor cron: find accounts whose scheduled deletion deadline has passed.
-- Partial index keeps it small — only rows with a pending deletion have entries.
CREATE INDEX IF NOT EXISTS accounts_gdpr_deletion_scheduled_idx
    ON accounts (gdpr_deletion_scheduled_at)
    WHERE gdpr_deletion_scheduled_at IS NOT NULL;
