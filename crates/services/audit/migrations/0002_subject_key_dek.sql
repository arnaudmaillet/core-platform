-- Envelope-encryption columns for the per-subject DEK (rationale-sealing). The
-- DEK is wrapped under the service KEK (which lives in the audit environment, not
-- here), so the row holds only the wrapped key — a DB operator alone cannot
-- decrypt. Crypto-shred (DELETE the row, via the key vault) destroys the wrapped
-- DEK and makes every envelope for that subject permanently undecryptable.
--
-- Nullable + additive: existing rows (e.g. bare key references) stay valid; the
-- cipher fills the columns on first use. Production swaps KEK custody to KMS/HSM
-- with no further schema change.
ALTER TABLE subject_keys ADD COLUMN IF NOT EXISTS wrapped_dek BYTEA;
ALTER TABLE subject_keys ADD COLUMN IF NOT EXISTS wrap_nonce BYTEA;
