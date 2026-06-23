-- Migration 0004: the bounded Member Plane roster
--
-- Partition key: conversation_id — the roster is bounded (<= 500 for a Group,
--   <= 200 broadcasters for a Channel), so a single partition is safe and lets
--   the application hydrate the whole roster in one token-aware read.
--
-- Clustering: member_id ASC — deterministic point lookups ("is X a member?")
--   and ordered full-roster scans.
--
-- last_read is the per-member read-receipt horizon (newest MessageId read). This
--   is the ONLY per-recipient state in the entire service and is bounded to
--   O(members) by construction — audience members generate no receipts, which is
--   the structural defence against read-receipt write-amplification.
--
-- Only member-plane roles (owner/admin/member) ever appear here; subscribers and
--   guests live in the subscriptions_* tables.
--
-- Compaction: LCS. Rows are point-read and UPDATEd in place (role changes,
--   last_read advances) and occasionally deleted (leave) — LCS keeps both read
--   amplification and tombstone spread low on this small, mutable partition.
CREATE TABLE IF NOT EXISTS chat.members_by_conversation (
    conversation_id uuid,
    member_id       uuid,
    role            tinyint,    -- 0=Owner, 1=Admin, 2=Member
    joined_at       timestamp,
    last_read       uuid,       -- read-receipt horizon (MessageId, UUID v7); NULL until first read
    PRIMARY KEY ((conversation_id), member_id)
) WITH CLUSTERING ORDER BY (member_id ASC)
  AND compaction  = {'class': 'LeveledCompactionStrategy'}
  AND compression = {'sstable_compression': 'LZ4Compressor'}
  AND gc_grace_seconds = 86400
  AND comment = 'Bounded member roster + read-receipt horizon. Member-plane roles only. Single-partition hydration.';
