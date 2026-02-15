# infrastructure/live/staging/us-east-1/kubernetes/eks/terragrunt.hcl

include "root" {
  path = find_in_parent_folders("root.hcl")
}

terraform {
  source = "../../../../../modules/kubernetes/eks"
}

dependency "vpc" {
  config_path = "../../networking"
}

locals {
  env_vars = read_terragrunt_config(find_in_parent_folders("env.hcl"))
}

inputs = {
  cluster_name = "core-platform-${local.env_vars.locals.env}"
  vpc_id       = dependency.vpc.outputs.vpc_id
  private_subnet_ids = dependency.vpc.outputs.private_app_subnet_ids

  # Injection des tailles d'instances dynamiques
  eks_instance_types_system   = local.env_vars.locals.eks_instance_types_system
  eks_instance_types_database = local.env_vars.locals.eks_instance_types_database
  eks_min_size                = local.env_vars.locals.eks_min_size
  eks_max_size                = local.env_vars.locals.eks_max_size
}