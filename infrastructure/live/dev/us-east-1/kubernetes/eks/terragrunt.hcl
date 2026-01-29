# infrastructure/live/dev/us-east-1/kubernetes/eks/terragrunt.hcl

include "root" {
  path = find_in_parent_folders("root.hcl")
}

terraform {
  source = "../../../../../modules/kubernetes/eks"
}

dependency "networking" {
  config_path = "../../networking"
}

inputs = {
  cluster_name       = "core-eks-dev"
  vpc_id             = dependency.networking.outputs.vpc_id
  private_subnet_ids = dependency.networking.outputs.private_app_subnet_ids
}