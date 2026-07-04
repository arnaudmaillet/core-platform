CREATE KEYSPACE IF NOT EXISTS geo_discovery
    WITH replication = {'class': 'NetworkTopologyStrategy', 'datacenter1': 3}
    AND durable_writes = true;
