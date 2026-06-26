-- Audit & compliance schema (Postgres). All three tables are append-only by
-- design; nothing in the service ever issues UPDATE or DELETE against the ledger.
--
-- PRODUCTION HARDENING (applied per-environment by ops, not here): grant the
-- service role INSERT + SELECT only and REVOKE UPDATE, DELETE on audit_records,
-- so even a compromised application credential cannot rewrite or remove a record.
-- The hash chain then makes tampering by a more-privileged operator *detectable*.
-- Left as a comment because the dev/test role is the table owner:
--   REVOKE UPDATE, DELETE ON audit_records FROM audit_role;

-- The canonical, hash-chained ledger. One row per record; the chain links
-- (record_hash / the prev_hash inside record_json) plus the monotonic per-partition
-- sequence_no make tampering and truncation detectable. record_json is the
-- serialized AuditRecord (the source of truth on read); the other columns are
-- denormalized for compare-and-append, idempotency and the query/verify paths.
CREATE TABLE IF NOT EXISTS audit_records (
    partition_key     TEXT     NOT NULL,
    sequence_no       BIGINT   NOT NULL,
    event_id          TEXT     NOT NULL UNIQUE,
    record_hash       TEXT     NOT NULL,
    subject_pseudonym TEXT,
    tenant_id         TEXT,
    category_tag      SMALLINT NOT NULL,
    occurred_at_ms    BIGINT   NOT NULL,
    recorded_at_ms    BIGINT   NOT NULL,
    record_json       TEXT     NOT NULL,
    PRIMARY KEY (partition_key, sequence_no)
);

-- Subject-scoped query / export / erasure index.
CREATE INDEX IF NOT EXISTS audit_subject_idx ON audit_records (subject_pseudonym);

-- Per-subject DEK custody (v1). Destroying a key (DELETE) is the crypto-shred that
-- erases a subject's PII while leaving the ledger intact. PRODUCTION: KMS/HSM in a
-- trust domain separate from this database.
CREATE TABLE IF NOT EXISTS subject_keys (
    key_ref       TEXT   PRIMARY KEY,
    created_at_ms BIGINT NOT NULL
);

-- Anchored Merkle checkpoints (v1). PRODUCTION: an RFC 3161 timestamp authority
-- and/or a cross-account WORM bucket — a witness outside the DB operator's control.
CREATE TABLE IF NOT EXISTS checkpoint_anchors (
    created_at_ms   BIGINT PRIMARY KEY,
    checkpoint_json TEXT   NOT NULL
);
