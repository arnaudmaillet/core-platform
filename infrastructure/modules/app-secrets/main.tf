# infrastructure/modules/app-secrets/main.tf
#
# Seeds the application secrets that the workload overlay's ExternalSecrets pull
# from AWS Secrets Manager but that NO other Terraform unit creates (previously
# "create out-of-band" — the gap that left media/audit/auth pods in
# CreateContainerConfigError). Generates everything so a from-scratch env needs
# zero manual steps:
#   * <name>-media-s3     {access_key, secret_key}                 (rusty-s3 static keys)
#   * <name>-scylla-s3    {access_key, secret_key}                 (scylla-manager-agent backups)
#   * <name>-audit-crypto {object/witness S3 keys, kek_base64, signing_key_base64}
#   * <name>-auth-secrets {ES256 signing PEM pair, keycloak_client_secret}
#
# STAGING v1 PATH: static IAM keys (rusty-s3 cannot use IRSA web-identity) and the
# env-KEK / signing key are GENERATED HERE and live in Terraform state. Prod's
# tamper-evidence story (real KMS/HSM custody, cross-account WORM witness) is the
# documented external deferral — see the audit blueprint; do NOT use this path for
# prod. The ESO read policy already covers <name>-* (modules/security/irsa-roles).
#
# NB: the tls (auth_signing) and random (KEK/signing/keycloak) providers are
# resolved implicitly by Terraform from the resource prefixes. They are NOT
# declared in a required_providers block here on purpose: Terragrunt generates the
# module's only versions.tf (aws + time, overwrite_terragrunt) and a module may
# have just one required_providers block.

# ── media: static S3 keys ─────────────────────────────────────────────────────
resource "aws_iam_user" "media" {
  name = "${var.name}-media-s3"
  tags = var.tags
}

resource "aws_iam_user_policy" "media" {
  name = "media-s3-rw"
  user = aws_iam_user.media.name
  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Sid      = "ListBucket"
        Effect   = "Allow"
        Action   = ["s3:ListBucket"]
        Resource = [var.media_bucket_arn]
      },
      {
        Sid      = "ObjectRW"
        Effect   = "Allow"
        Action   = ["s3:GetObject", "s3:PutObject", "s3:DeleteObject", "s3:AbortMultipartUpload"]
        Resource = ["${var.media_bucket_arn}/*"]
      },
    ]
  })
}

resource "aws_iam_access_key" "media" {
  user = aws_iam_user.media.name
}

resource "aws_secretsmanager_secret" "media_s3" {
  name                    = "${var.name}-media-s3"
  recovery_window_in_days = var.secret_recovery_window_days
  tags                    = var.tags
}

resource "aws_secretsmanager_secret_version" "media_s3" {
  secret_id = aws_secretsmanager_secret.media_s3.id
  secret_string = jsonencode({
    access_key = aws_iam_access_key.media.id
    secret_key = aws_iam_access_key.media.secret
  })
}

# ── scylla: static S3 keys for the Scylla Manager agent (backups) ─────────────
# Unlike audit's append-only users, the agent PURGES snapshots past the backup
# task's retention, so it needs delete on the backup bucket. Keys reach the
# `scylla` namespace as scylla-agent-config-secret via an ExternalSecret
# (k8s/base/infra/scylla-cluster).
resource "aws_iam_user" "scylla" {
  name = "${var.name}-scylla-s3"
  tags = var.tags
}

resource "aws_iam_user_policy" "scylla" {
  name = "scylla-backups-rw"
  user = aws_iam_user.scylla.name
  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Sid      = "ListBucket"
        Effect   = "Allow"
        Action   = ["s3:ListBucket", "s3:GetBucketLocation"]
        Resource = [var.scylla_backups_bucket_arn]
      },
      {
        Sid      = "SnapshotRW"
        Effect   = "Allow"
        Action   = ["s3:GetObject", "s3:PutObject", "s3:DeleteObject", "s3:AbortMultipartUpload", "s3:ListMultipartUploadParts"]
        Resource = ["${var.scylla_backups_bucket_arn}/*"]
      },
    ]
  })
}

resource "aws_iam_access_key" "scylla" {
  user = aws_iam_user.scylla.name
}

resource "aws_secretsmanager_secret" "scylla_s3" {
  name                    = "${var.name}-scylla-s3"
  recovery_window_in_days = var.secret_recovery_window_days
  tags                    = var.tags
}

resource "aws_secretsmanager_secret_version" "scylla_s3" {
  secret_id = aws_secretsmanager_secret.scylla_s3.id
  secret_string = jsonencode({
    access_key = aws_iam_access_key.scylla.id
    secret_key = aws_iam_access_key.scylla.secret
  })
}

# ── audit: static S3 keys (object store + witness) + crypto custody (v1) ───────
# Two independent users so object-store and witness credentials rotate separately.
# Both scoped to the WORM bucket, append-only (Object-Lock blocks deletes anyway),
# plus KMS to write/read the SSE-KMS objects under the audit KEK.
resource "aws_iam_user" "audit_object" {
  name = "${var.name}-audit-object"
  tags = var.tags
}

resource "aws_iam_user" "audit_witness" {
  name = "${var.name}-audit-witness"
  tags = var.tags
}

resource "aws_iam_user_policy" "audit_object" {
  name   = "audit-worm-append"
  user   = aws_iam_user.audit_object.name
  policy = local.audit_worm_policy
}

resource "aws_iam_user_policy" "audit_witness" {
  name   = "audit-worm-append"
  user   = aws_iam_user.audit_witness.name
  policy = local.audit_worm_policy
}

locals {
  # Append-only write + read to the WORM bucket; GenerateDataKey/Decrypt on the
  # audit KEK for SSE-KMS. No s3:DeleteObject (the ledger never deletes).
  audit_worm_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      { Sid = "ListBucket", Effect = "Allow", Action = ["s3:ListBucket"], Resource = [var.audit_worm_bucket_arn] },
      { Sid = "ObjectAppend", Effect = "Allow", Action = ["s3:GetObject", "s3:PutObject"], Resource = ["${var.audit_worm_bucket_arn}/*"] },
      { Sid = "KmsForSse", Effect = "Allow", Action = ["kms:GenerateDataKey", "kms:Decrypt"], Resource = [var.audit_kms_key_arn] },
    ]
  })
}

resource "aws_iam_access_key" "audit_object" {
  user = aws_iam_user.audit_object.name
}

resource "aws_iam_access_key" "audit_witness" {
  user = aws_iam_user.audit_witness.name
}

# App-level env KEK (wraps per-subject DEKs for crypto-shred) + checkpoint signing
# key. 32 random bytes each, base64. v1 only — prod custody = real KMS/HSM.
resource "random_bytes" "audit_kek" {
  length = 32
}

resource "random_bytes" "audit_signing_key" {
  length = 32
}

resource "aws_secretsmanager_secret" "audit_crypto" {
  name                    = "${var.name}-audit-crypto"
  recovery_window_in_days = var.secret_recovery_window_days
  tags                    = var.tags
}

resource "aws_secretsmanager_secret_version" "audit_crypto" {
  secret_id = aws_secretsmanager_secret.audit_crypto.id
  secret_string = jsonencode({
    object_access_key  = aws_iam_access_key.audit_object.id
    object_secret_key  = aws_iam_access_key.audit_object.secret
    witness_access_key = aws_iam_access_key.audit_witness.id
    witness_secret_key = aws_iam_access_key.audit_witness.secret
    kek_base64         = random_bytes.audit_kek.base64
    signing_key_base64 = random_bytes.audit_signing_key.base64
  })
}

# ── auth: ES256 signing keypair + Keycloak client secret (placeholder) ────────
# Keycloak is not yet provisioned (auth prerequisite), so the client secret is a
# generated placeholder until it lands.
resource "tls_private_key" "auth_signing" {
  algorithm   = "ECDSA"
  ecdsa_curve = "P256"
}

resource "random_password" "keycloak_client_secret" {
  length  = 40
  special = false
}

resource "aws_secretsmanager_secret" "auth_secrets" {
  name                    = "${var.name}-auth-secrets"
  recovery_window_in_days = var.secret_recovery_window_days
  tags                    = var.tags
}

resource "aws_secretsmanager_secret_version" "auth_secrets" {
  secret_id = aws_secretsmanager_secret.auth_secrets.id
  secret_string = jsonencode({
    signing_private_pem    = tls_private_key.auth_signing.private_key_pem
    signing_public_pem     = tls_private_key.auth_signing.public_key_pem
    keycloak_client_secret = random_password.keycloak_client_secret.result
  })
}
