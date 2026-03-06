# infrastructure/live/dev/us-east-1/kubernetes/02-argocd-bootstrap/terragrunt.hcl

include "root" {
  path = find_in_parent_folders("root.hcl")
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

terraform {
  source = "../../../../../modules//kubernetes/argocd-bootstrap"
}

inputs = {
  cluster_name           = dependency.eks.outputs.cluster_name
  cluster_endpoint       = dependency.eks.outputs.cluster_endpoint
  cluster_ca_certificate = dependency.eks.outputs.cluster_certificate_authority_data
  ssl_certificate_arn    = dependency.identity.outputs.certificate_arn
  addons_iam_roles = dependency.identity.outputs.iam_role_arns
}