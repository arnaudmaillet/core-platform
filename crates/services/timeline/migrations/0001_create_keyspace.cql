CREATE KEYSPACE IF NOT EXISTS timeline
    WITH replication = {'class': 'NetworkTopologyStrategy', 'datacenter1': 3}
    AND durable_writes = true;
