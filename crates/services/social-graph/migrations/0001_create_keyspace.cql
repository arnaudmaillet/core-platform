CREATE KEYSPACE IF NOT EXISTS social_graph
    WITH replication = {'class': 'NetworkTopologyStrategy', 'datacenter1': 3}
    AND durable_writes = true;
