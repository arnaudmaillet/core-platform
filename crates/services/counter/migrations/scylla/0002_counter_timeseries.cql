-- Historical per-bucket rollups. A `counter` column accumulates each window's
-- scalar contribution into its hour bucket; TWCS keeps time-ordered SSTables
-- cheap to expire. Read only by GetTimeSeries (off the hot path).
CREATE TABLE IF NOT EXISTS counter.timeseries (
    entity_kind  text,
    entity_id    text,
    metric       text,
    bucket_start timestamp,
    value        counter,
    PRIMARY KEY ((entity_kind, entity_id, metric), bucket_start)
) WITH compaction = {'class': 'TimeWindowCompactionStrategy'};
