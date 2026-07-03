# infrastructure/live/prod/us-east-1/data/media-bucket/terragrunt.hcl
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
  # Browser presigned PUT/GET only from the real web origins.
  cors_allowed_origins = ["https://core-platform.click", "https://www.core-platform.click"]
  # Prod media is user data: destroy must never be able to empty the bucket.
  force_destroy = false

  tags = {
    Environment = local.env_vars.locals.env
    ManagedBy   = "terragrunt"
    Service     = "media"
  }
}
