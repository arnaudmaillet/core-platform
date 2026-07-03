# infrastructure/live/prod/us-east-1/data/app-secrets/terragrunt.hcl
#
# Seeds the app secrets the workload ExternalSecrets need but no other unit
# creates (media-s3 / scylla-s3 / audit-crypto / auth-secrets) — closing the
# "create out-of-band" gap that left media/audit/auth pods in
# CreateContainerConfigError. scylla-s3 feeds the scylla-manager-agent backup
# credentials (k8s/base/infra/scylla-cluster-prod ExternalSecret).
#
# ⚠ PROD CUSTODY DEBT (tracked, do not consider this unit "done"): this seeds
# the SAME v1 path as staging — static IAM keys and a Terraform-state-resident
# KEK/signing key. The audit blueprint's prod story is real KMS/HSM custody and
# a cross-account WORM witness; until that lands, prod's tamper-evidence rests
# on state-file access control. Wired anyway so the env boots end-to-end.
# Secrets use the default (recoverable) recovery window here — nothing in prod
# is disposable.

include "root" {
  path = find_in_parent_folders("root.hcl")
}

terraform {
  source = "../../../../../modules/app-secrets"
}

dependency "media_bucket" {
  config_path                             = "../media-bucket"
  mock_outputs                            = { bucket_arn = "arn:aws:s3:::mock-media" }
  mock_outputs_allowed_terraform_commands = ["validate", "plan"]
}

dependency "audit_worm" {
  config_path                             = "../audit-worm"
  mock_outputs                            = { bucket_arn = "arn:aws:s3:::mock-audit-worm" }
  mock_outputs_allowed_terraform_commands = ["validate", "plan"]
}

dependency "audit_kms" {
  config_path                             = "../audit-kms"
  mock_outputs                            = { key_arn = "arn:aws:kms:us-east-1:000000000000:key/mock" }
  mock_outputs_allowed_terraform_commands = ["validate", "plan"]
}

dependency "scylla_backups" {
  config_path                             = "../scylla-backups"
  mock_outputs                            = { bucket_arn = "arn:aws:s3:::mock-scylla-backups" }
  mock_outputs_allowed_terraform_commands = ["validate", "plan"]
}

locals {
  env_vars = read_terragrunt_config(find_in_parent_folders("env.hcl"))
}

inputs = {
  name                      = "core-platform-${local.env_vars.locals.env}"
  media_bucket_arn          = dependency.media_bucket.outputs.bucket_arn
  audit_worm_bucket_arn     = dependency.audit_worm.outputs.bucket_arn
  audit_kms_key_arn         = dependency.audit_kms.outputs.key_arn
  scylla_backups_bucket_arn = dependency.scylla_backups.outputs.bucket_arn

  tags = {
    Environment = local.env_vars.locals.env
    ManagedBy   = "terragrunt"
  }
}
