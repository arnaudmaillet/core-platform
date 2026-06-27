-- Migration: 0001_create_moderation_history (ScyllaDB)
-- Description: The high-volume, append-only evidence/history time-series — the
--              denormalized audit feed projected from the moderation event stream.
--              Postgres is the decision/case system of record; this store retains
--              the firehose of moderation events per actor for transparency
--              reporting, law-enforcement preservation, and model training.
--              Bucketed and TWCS-compacted: write-heavy, time-ordered, never
--              updated in place.

CREATE KEYSPACE IF NOT EXISTS moderation
    WITH replication = {'class': 'NetworkTopologyStrategy', 'datacenter1': 3};

-- One row per moderation event, clustered newest-first within an actor's feed.
-- `event` holds the serialized DomainEvent JSON; the typed columns make the feed
-- queryable by time/category without deserializing every row.
-- event_id is a UUIDv7 (time-ordered) tiebreaker — avoids v1/timeuuid.
CREATE TABLE IF NOT EXISTS moderation.evidence_history (
    actor_id     uuid,
    occurred_at  timestamp,
    event_id     uuid,
    event_type   text,
    entity_type  text,
    entity_id    text,
    category     text,
    action       text,
    event        text,
    PRIMARY KEY ((actor_id), occurred_at, event_id)
) WITH CLUSTERING ORDER BY (occurred_at DESC, event_id DESC)
  AND compaction = {'class': 'TimeWindowCompactionStrategy',
                    'compaction_window_unit': 'DAYS',
                    'compaction_window_size': 1};
