CREATE KEYSPACE IF NOT EXISTS comment
    WITH replication = {'class': 'NetworkTopologyStrategy', 'datacenter1': 3}
    AND durable_writes = true;
