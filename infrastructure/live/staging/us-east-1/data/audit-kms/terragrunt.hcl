# infrastructure/live/staging/us-east-1/data/audit-kms/terragrunt.hcl
#
# Audit KEK: wraps audit's per-subject DEKs. The audit-server IRSA role is the
# SOLE principal granted kms:Decrypt/GenerateDataKey on this key (Block 4). GDPR
# crypto-shred = destroy the per-subject DEK; this KEK is never exported.

include "root" {
  path = find_in_parent_folders("root.hcl")
}

terraform {
  source = "../../../../../modules/kms-key"
}

locals {
  env_vars = read_terragrunt_config(find_in_parent_folders("env.hcl"))
}

inputs = {
  alias               = "core-platform-${local.env_vars.locals.env}-audit-kek"
  description         = "Audit KEK — wraps per-subject DEKs for the audit evidence ledger."
  enable_key_rotation = true

  tags = {
    Environment = local.env_vars.locals.env
    ManagedBy   = "terragrunt"
    Service     = "audit"
  }
}
