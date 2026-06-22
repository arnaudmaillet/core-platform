-- Migration 001: notification keyspace
-- NetworkTopologyStrategy with RF=3 for datacenter1.
-- durable_writes must remain true — counter table writes are not journaled
-- at the client level, so the commit log is the sole durability guarantee.

CREATE KEYSPACE IF NOT EXISTS notification
  WITH replication = {
    'class'          : 'NetworkTopologyStrategy',
    'datacenter1'    : 3
  }
  AND durable_writes = true;
