# infrastructure/live/prod/us-east-1/data/cnpg-backups/terragrunt.hcl
#
# Shared CNPG backup target: base backups + WAL for every CNPG cluster, one path
# prefix per cluster (s3://<bucket>/<cluster>). Versioned, SSE-S3, private. The
# CNPG pods write via the cnpg-backup IRSA role (Block: security/irsa-roles).
# Bucket name is account-scoped for S3 global uniqueness.

include "root" {
  path = find_in_parent_folders("root.hcl")
}

terraform {
  source = "../../../../../modules/s3-bucket"
}

locals {
  env_vars   = read_terragrunt_config(find_in_parent_folders("env.hcl"))
  account_id = get_aws_account_id()
}

inputs = {
  name               = "core-platform-${local.env_vars.locals.env}-cnpg-backups-${local.account_id}"
  versioning_enabled = true
  # Object-store WAL/base backups; no Object-Lock (retention is managed by CNPG's
  # barman retentionPolicy, which must be able to prune).
  cors_enabled = false
  # Prod backups are the recovery story — destroy must never empty this bucket.
  force_destroy = false

  tags = {
    Environment = local.env_vars.locals.env
    ManagedBy   = "terragrunt"
    Service     = "cnpg-backups"
  }
}
