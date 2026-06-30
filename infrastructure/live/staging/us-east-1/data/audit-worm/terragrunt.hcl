# infrastructure/live/staging/us-east-1/data/audit-worm/terragrunt.hcl
#
# Audit WORM anchor bucket: Object-Lock + SSE-KMS under the audit KEK. This is the
# external-witness sink for the signed Merkle checkpoints. audit-server is the
# only principal granted write (no delete) — see Block 4 IRSA.
#
# STAGING POSTURE: GOVERNANCE (not COMPLIANCE) + force_destroy. Staging is an
# ephemeral, frequently-rebuilt environment; COMPLIANCE makes the bucket literally
# undeletable for the full retention (7y) even by root, which hard-stalls
# `terragrunt run --all destroy`. GOVERNANCE preserves WORM semantics for the
# audit service at runtime (audit's IRSA role has NO bypass permission, so it
# still cannot delete/overwrite checkpoints) while letting the teardown principal
# empty + delete the bucket via force_destroy — the AWS provider sets
# BypassGovernanceRetention on the delete calls, which requires the destroying
# principal to hold s3:BypassGovernanceRetention. PROD must flip this back to
# COMPLIANCE + force_destroy=false (true tamper-evidence; never disposable).

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
  # GOVERNANCE so staging stays disposable (see header); PROD => COMPLIANCE.
  object_lock_mode           = "GOVERNANCE"
  object_lock_retention_days = 2555 # ~7 years
  kms_key_arn                = dependency.audit_kms.outputs.key_arn

  # Let teardown empty + delete the bucket. With GOVERNANCE this works because the
  # provider passes BypassGovernanceRetention on object deletes (the destroying
  # principal needs s3:BypassGovernanceRetention). PROD => false.
  force_destroy = true

  tags = {
    Environment = local.env_vars.locals.env
    ManagedBy   = "terragrunt"
    Service     = "audit"
  }
}
