# infrastructure/live/dev/us-east-1/kubernetes/01-argocd-server/terragrunt.hcl

include "root" {
  path = find_in_parent_folders("root.hcl")
}

terraform {
  source = "../../../../../modules//kubernetes/argocd-server" # Module qui contient UNIQUEMENT le helm_release
}

dependency "eks" {
  config_path = "../../eks"
}

inputs = {
  cluster_name           = dependency.eks.outputs.cluster_name
  cluster_endpoint       = dependency.eks.outputs.cluster_endpoint
  cluster_ca_certificate = dependency.eks.outputs.cluster_certificate_authority_data
}