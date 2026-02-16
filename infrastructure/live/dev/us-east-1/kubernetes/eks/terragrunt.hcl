# infrastructure/live/dev/us-east-1/kubernetes/eks/terragrunt.hcl

include "root" {
  path = find_in_parent_folders("root.hcl")
}

terraform {
  source = "../../../../../modules/kubernetes/eks"
}

dependency "vpc" {
  config_path = "../../networking/vpc"
}

locals {
  env_vars = read_terragrunt_config(find_in_parent_folders("env.hcl"))
}

inputs = {
  cluster_name       = "core-platform-${local.env_vars.locals.env}"
  vpc_id             = dependency.vpc.outputs.vpc_id
  private_subnet_ids = dependency.vpc.outputs.private_app_subnet_ids

  # EKS Node Groups
  system_node_settings = local.env_vars.locals.system_node_settings
  mgmt_node_settings   = local.env_vars.locals.mgmt_node_settings
  db_node_settings     = local.env_vars.locals.db_node_settings

  iam_policy_json_content = file("iam_policy.json")
  project_name            = "core-platform"
  env                     = local.env_vars.locals.env
}