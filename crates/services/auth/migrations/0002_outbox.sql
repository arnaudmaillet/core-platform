-- Transactional-outbox table for auth's domain events (auth.v1.events).
--
-- WHY: Login/refresh persisted state and then AWAITED the Kafka publish inside
-- the RPC — TIER-0 login availability was coupled to broker availability (the
-- E2E drill found logins hanging to deadline on a misconfigured broker), and a
-- failed publish after a committed session silently dropped COMPLIANCE
-- evidence (audit consumes auth.v1.events). Handlers now enqueue here — the
-- same fault domain as the session writes themselves — and a background relay
-- drains to Kafka with retries.
--
-- Rows are DELETED on successful publish: the table holds only the pending
-- backlog (the durable evidence trail lives downstream in the audit ledger).
-- id is a UUIDv7, so (created_at, id) gives a stable time-ordered drain that
-- preserves per-account Kafka partition ordering.
CREATE TABLE IF NOT EXISTS auth_outbox (
    id          UUID        PRIMARY KEY,
    event_type  TEXT        NOT NULL,
    payload     JSONB       NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS auth_outbox_drain_order
    ON auth_outbox (created_at, id);
