# infrastructure/live/staging/us-east-1/data/media-bucket/terragrunt.hcl
#
# Media asset object store: versioned, SSE-S3, CORS for browser presigned
# upload/download. Bucket name is account-scoped for S3 global uniqueness.

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
  name               = "core-platform-${local.env_vars.locals.env}-media-${local.account_id}"
  versioning_enabled = true
  cors_enabled       = true
  # Tighten to the real web origins before prod; '*' is acceptable for staging.
  cors_allowed_origins = ["*"]
  # Staging media is ephemeral test data — let destroy clean the bucket. (Prod
  # would keep this false.)
  force_destroy = true

  tags = {
    Environment = local.env_vars.locals.env
    ManagedBy   = "terragrunt"
    Service     = "media"
  }
}
