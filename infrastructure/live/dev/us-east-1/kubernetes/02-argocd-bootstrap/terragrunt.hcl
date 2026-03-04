# infrastructure/live/dev/us-east-1/kubernetes/02-argocd-bootstrap/terragrunt.hcl

include "root" {
  path = find_in_parent_folders("root.hcl")
}

terraform {
  source = "../../../../../modules//kubernetes/argocd-bootstrap" # Module qui contient UNIQUEMENT le kubernetes_manifest
}

dependency "eks" {
  config_path = "../../eks"
}

dependency "identity" {
  config_path = "../identity"
}

dependency "argocd_server" {
  config_path = "../01-argocd-server"
}

inputs = {
  cluster_name           = dependency.eks.outputs.cluster_name
  cluster_endpoint       = dependency.eks.outputs.cluster_endpoint
  cluster_ca_certificate = dependency.eks.outputs.cluster_certificate_authority_data

  # Injection des rôles IAM provenant du module Identity
  addons_iam_roles = dependency.identity.outputs.iam_role_arns
}