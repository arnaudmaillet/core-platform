-- Cold time-series tier keyspace. The test harness rewrites this to
-- SimpleStrategy RF=1 for a single-node container; production runs the
-- NetworkTopologyStrategy below.
CREATE KEYSPACE IF NOT EXISTS counter
    WITH replication = {'class': 'NetworkTopologyStrategy', 'datacenter1': 3}
    AND durable_writes = true;
