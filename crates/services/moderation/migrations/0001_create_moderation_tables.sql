-- Migration: 0001_create_moderation_tables
-- Description: Durable system of record for the moderation bounded context — the
--              review-case workbench, the append-only decision ledger (the legal
--              evidence record), the enforcement-action lifecycle, the per-actor
--              penalty ledger, and the appeal lifecycle. Pure ANSI SQL, portable
--              across PostgreSQL and CockroachDB. Every table carries an actor_id
--              so an actor's moderation state co-locates on one shard.

-- ── Cases ────────────────────────────────────────────────────────────────────
-- One row per review subject. `id` is the deterministic UUIDv5 of the subject, so
-- a redelivered content event upserts the same case instead of duplicating.
-- Accrued evidence signals are stored as JSON (they are an unbounded list read as
-- a whole, never queried field-wise).
CREATE TABLE IF NOT EXISTS cases (
    id            UUID         NOT NULL,
    entity_type   TEXT         NOT NULL,
    entity_id     TEXT         NOT NULL,
    actor_id      UUID         NOT NULL,
    surface       TEXT         NOT NULL,
    status        TEXT         NOT NULL DEFAULT 'open',
    category      TEXT         NOT NULL,
    queue         TEXT         NOT NULL,
    priority      TEXT         NOT NULL,
    assignee      TEXT,
    signals       JSONB        NOT NULL DEFAULT '[]'::jsonb,
    opened_at     TIMESTAMPTZ  NOT NULL,
    version       BIGINT       NOT NULL DEFAULT 0,
    PRIMARY KEY (id)
);

-- The triage queue read path: open work in a queue, newest first.
CREATE INDEX IF NOT EXISTS idx_cases_queue_status
    ON cases (queue, status, opened_at DESC);

-- ── Decisions (append-only ledger) ───────────────────────────────────────────
-- The legal evidence record. Rows are NEVER updated; a reversal is a new row that
-- references the one it supersedes via `reverses`.
CREATE TABLE IF NOT EXISTS decisions (
    id             UUID         NOT NULL,
    entity_type    TEXT         NOT NULL,
    entity_id      TEXT         NOT NULL,
    actor_id       UUID         NOT NULL,
    surface        TEXT         NOT NULL,
    action         TEXT         NOT NULL,
    category       TEXT         NOT NULL,
    policy_version TEXT         NOT NULL,
    rationale      TEXT         NOT NULL,
    author_kind    TEXT         NOT NULL, -- 'reviewer' | 'rule'
    author_id      TEXT         NOT NULL,
    reverses       UUID,
    decided_at     TIMESTAMPTZ  NOT NULL,
    PRIMARY KEY (id)
);

-- Transparency reporting & an actor's decision history.
CREATE INDEX IF NOT EXISTS idx_decisions_actor      ON decisions (actor_id, decided_at DESC);
CREATE INDEX IF NOT EXISTS idx_decisions_category   ON decisions (category, decided_at DESC);

-- ── Enforcement actions ──────────────────────────────────────────────────────
-- The executable consequence of a decision. `version` is monotonic per subject so
-- a reversal can never race ahead of a newer re-application.
CREATE TABLE IF NOT EXISTS enforcements (
    id            UUID         NOT NULL,
    entity_type   TEXT         NOT NULL,
    entity_id     TEXT         NOT NULL,
    actor_id      UUID         NOT NULL,
    surface       TEXT         NOT NULL,
    action        TEXT         NOT NULL,
    status        TEXT         NOT NULL DEFAULT 'active',
    version       BIGINT       NOT NULL,
    decision_id   UUID         NOT NULL,
    applied_at    TIMESTAMPTZ  NOT NULL,
    expires_at    TIMESTAMPTZ,
    reversed_at   TIMESTAMPTZ,
    PRIMARY KEY (id)
);

-- next_version(subject): MAX(version) over a subject's enforcements.
CREATE INDEX IF NOT EXISTS idx_enforcements_subject
    ON enforcements (entity_type, entity_id, surface, version DESC);
-- Active enforcements for an actor (GetEnforcementState / appeal reversal).
CREATE INDEX IF NOT EXISTS idx_enforcements_actor_status
    ON enforcements (actor_id, status);

-- ── Penalty ledgers ──────────────────────────────────────────────────────────
-- One row per actor; the graduated-enforcement strike history (each strike's
-- snapshotted points + decay deadline) is stored as JSON and evaluated in-domain.
CREATE TABLE IF NOT EXISTS penalty_ledgers (
    actor_id      UUID         NOT NULL,
    strikes       JSONB        NOT NULL DEFAULT '[]'::jsonb,
    version       BIGINT       NOT NULL DEFAULT 0,
    PRIMARY KEY (actor_id)
);

-- ── Appeals ──────────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS appeals (
    id            UUID         NOT NULL,
    decision_id   UUID         NOT NULL,
    actor_id      UUID         NOT NULL,
    statement     TEXT         NOT NULL,
    status        TEXT         NOT NULL DEFAULT 'filed',
    filed_at      TIMESTAMPTZ  NOT NULL,
    resolved_at   TIMESTAMPTZ,
    PRIMARY KEY (id)
);

CREATE INDEX IF NOT EXISTS idx_appeals_decision ON appeals (decision_id);
