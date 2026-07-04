-- Migration 0001: chat keyspace
-- NetworkTopologyStrategy with RF=3 for datacenter1, matching the other services.
-- durable_writes stays true: the message log is the durable source of truth for
-- both the Member Plane and the Audience Plane (the Redis hot-tail cache and the
-- broadcast planes are best-effort derivations of it).

CREATE KEYSPACE IF NOT EXISTS chat
  WITH replication = {
    'class'       : 'NetworkTopologyStrategy',
    'datacenter1' : 3
  }
  AND durable_writes = true;
