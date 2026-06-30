# infrastructure/live/staging/us-east-1/data/elasticache/terragrunt.hcl

include "root" {
  path = find_in_parent_folders("root.hcl")
}

dependency "vpc" {
  config_path = "../../networking/vpc"
}

terraform {
  source = "../../../../../modules/elasticache"
}

locals {
  env_vars = read_terragrunt_config(find_in_parent_folders("env.hcl"))
}

inputs = {
  name                = "core-platform-${local.env_vars.locals.env}"
  vpc_id              = dependency.vpc.outputs.vpc_id
  subnet_ids          = dependency.vpc.outputs.private_data_subnet_ids
  allowed_cidr_blocks = [dependency.vpc.outputs.vpc_cidr_block]

  # Disposable staging: drop the Redis-auth secret immediately on destroy so a
  # rebuild doesn't collide with its recovery window. PROD => omit (recoverable).
  secret_recovery_window_days = 0

  tags = {
    Environment = local.env_vars.locals.env
    ManagedBy   = "terragrunt"
  }
}
