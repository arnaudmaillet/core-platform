# infrastructure/modules/kms-key/main.tf
#
# A single customer-managed KMS key + human-readable alias. Provisioned standalone
# (its own state dir) so the consuming service's IRSA policy can scope kms:Decrypt
# / kms:GenerateDataKey to exactly this key ARN — key custody stays in the
# service's trust domain.
#
# Used for the audit KEK: audit wraps its per-subject DEKs under this key; GDPR
# crypto-shred = destroy the DEK, the KEK is never exposed outside audit's role.
# Key-policy administration is left to the account root + the caller's IAM; this
# module deliberately does NOT grant broad usage — the grant is the consumer's
# IRSA policy (see modules/security/irsa-roles), so access is auditable in one place.

resource "aws_kms_key" "this" {
  description             = var.description
  deletion_window_in_days = var.deletion_window_in_days
  enable_key_rotation     = var.enable_key_rotation
  tags                    = var.tags
}

resource "aws_kms_alias" "this" {
  name          = "alias/${var.alias}"
  target_key_id = aws_kms_key.this.key_id
}
