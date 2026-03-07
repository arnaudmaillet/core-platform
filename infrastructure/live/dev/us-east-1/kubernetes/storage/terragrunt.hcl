# infrastructure/live/dev/us-east-1/kubernetes/storage/terragrunt.hcl

include "root" {
  path = find_in_parent_folders("root.hcl")
}

dependency "eks" {
  config_path = "../../eks"
}

dependency "identity" {
  config_path = "../../security/irsa-roles"
}

terraform {
  source = "../../../../../modules//kubernetes/storage"
}

inputs = {
  cluster_name     = dependency.eks.outputs.cluster_name
  ebs_csi_role_arn = dependency.identity.outputs.ebs_csi_role_arn
  cluster_endpoint       = dependency.eks.outputs.cluster_endpoint
  cluster_ca_certificate = dependency.eks.outputs.cluster_certificate_authority_data
}