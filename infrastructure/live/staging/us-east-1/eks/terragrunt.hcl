# infrastructure/live/staging/us-east-1/eks/terragrunt.hcl
# Mirrors the dev EKS component with staging's node_groups (env.hcl).

include "root" {
  path = find_in_parent_folders("root.hcl")
}

terraform {
  source = "../../../../modules/eks"
}

dependency "vpc" {
  config_path = "../networking/vpc"
}

locals {
  env_vars = read_terragrunt_config(find_in_parent_folders("env.hcl"))
}

inputs = {
  cluster_name       = "core-platform-${local.env_vars.locals.env}"
  vpc_id             = dependency.vpc.outputs.vpc_id
  private_subnet_ids = dependency.vpc.outputs.private_app_subnet_ids
  node_groups        = local.env_vars.locals.node_groups

  project_name = "core-platform"
  env          = local.env_vars.locals.env
}
