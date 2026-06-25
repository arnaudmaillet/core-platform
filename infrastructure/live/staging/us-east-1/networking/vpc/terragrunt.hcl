# infrastructure/live/staging/us-east-1/networking/vpc/terragrunt.hcl
# Mirrors the dev VPC component with staging's CIDR (env.hcl: 10.20.0.0/16).

include "root" {
  path   = find_in_parent_folders("root.hcl")
  expose = true
}

terraform {
  source = "../../../../../modules/networking/vpc"
}

locals {
  env_vars    = read_terragrunt_config(find_in_parent_folders("env.hcl"))
  region_vars = read_terragrunt_config(find_in_parent_folders("region.hcl"))
}

inputs = {
  cluster_name       = "core-platform-${local.env_vars.locals.env}"
  vpc_cidr           = local.env_vars.locals.vpc_cidr
  availability_zones = ["${local.region_vars.locals.aws_region}a", "${local.region_vars.locals.aws_region}b"]
}
