# infrastructure/live/staging/us-east-1/data/app-secrets/terragrunt.hcl
#
# Seeds the 3 app secrets the workload ExternalSecrets need but no other unit
# created (media-s3 / audit-crypto / auth-secrets) — closing the "create
# out-of-band" gap that left media/audit/auth pods in CreateContainerConfigError.
# STAGING v1: static IAM keys + generated KEK/signing/ES256 in TF state. Prod uses
# real KMS/HSM custody (deferred — see audit blueprint). secret_recovery_window=0
# keeps staging disposable (same posture as the data-store secrets).

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

locals {
  env_vars = read_terragrunt_config(find_in_parent_folders("env.hcl"))
}

inputs = {
  name                  = "core-platform-${local.env_vars.locals.env}"
  media_bucket_arn      = dependency.media_bucket.outputs.bucket_arn
  audit_worm_bucket_arn = dependency.audit_worm.outputs.bucket_arn
  audit_kms_key_arn     = dependency.audit_kms.outputs.key_arn

  # Disposable staging: free the secret names immediately on destroy.
  secret_recovery_window_days = 0

  tags = {
    Environment = local.env_vars.locals.env
    ManagedBy   = "terragrunt"
  }
}
