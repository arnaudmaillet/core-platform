-- Warm counter tier (Postgres): the auditable durable totals + the idempotency
-- ledger. Written only by the worker's batched window flushes; read on a
-- cache-aside miss and by reconciliation.

-- One row per flushed window. Its existence is the idempotency token: a
-- redelivered (entity, metric, window_id) hits the PK and is skipped, so the
-- total is never double-advanced.
CREATE TABLE IF NOT EXISTS counter_windows (
    entity_kind TEXT   NOT NULL,
    entity_id   TEXT   NOT NULL,
    metric      TEXT   NOT NULL,
    window_id   BIGINT NOT NULL,
    PRIMARY KEY (entity_kind, entity_id, metric, window_id)
);

-- The materialized durable total per (entity, metric) — the auditable
-- "how many", and the cache-aside fallback when the hot tier is cold.
CREATE TABLE IF NOT EXISTS counter_totals (
    entity_kind TEXT   NOT NULL,
    entity_id   TEXT   NOT NULL,
    metric      TEXT   NOT NULL,
    total       BIGINT NOT NULL,
    PRIMARY KEY (entity_kind, entity_id, metric)
);
