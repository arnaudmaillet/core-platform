# infrastructure/live/staging/us-east-1/data/audit-worm/terragrunt.hcl
#
# Audit WORM anchor bucket: Object-Lock COMPLIANCE (true write-once — not even
# root can delete/overwrite before retention), SSE-KMS under the audit KEK. This
# is the external-witness sink for the signed Merkle checkpoints. audit-server is
# the only principal granted write (no delete) — see Block 4 IRSA.

include "root" {
  path = find_in_parent_folders("root.hcl")
}

terraform {
  source = "../../../../../modules/s3-bucket"
}

dependency "audit_kms" {
  config_path = "../audit-kms"

  # Lets `plan` run before the KMS key exists (e.g. first run-all).
  mock_outputs = {
    key_arn = "arn:aws:kms:us-east-1:000000000000:key/mock"
  }
  mock_outputs_allowed_terraform_commands = ["validate", "plan"]
}

locals {
  env_vars   = read_terragrunt_config(find_in_parent_folders("env.hcl"))
  account_id = get_aws_account_id()
}

inputs = {
  name = "core-platform-${local.env_vars.locals.env}-audit-worm-${local.account_id}"

  # WORM: versioning is forced on by the module when object_lock_mode is set.
  object_lock_mode           = "COMPLIANCE"
  object_lock_retention_days = 2555 # ~7 years
  kms_key_arn                = dependency.audit_kms.outputs.key_arn

  tags = {
    Environment = local.env_vars.locals.env
    ManagedBy   = "terragrunt"
    Service     = "audit"
  }
}
