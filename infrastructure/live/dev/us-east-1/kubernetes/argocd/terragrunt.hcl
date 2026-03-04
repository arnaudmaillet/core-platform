# infrastructure/live/dev/us-east-1/kubernetes/argocd/terragrunt.hcl

include "root" {
  path = find_in_parent_folders("root.hcl")
}

terraform {
  source = "../../../../../modules//kubernetes/argocd"
}

dependency "eks" {
  config_path = "../../eks"
}

dependency "identity" {
  config_path = "../identity"
}

inputs = {
  cluster_name = dependency.eks.outputs.cluster_name
  cluster_endpoint       = dependency.eks.outputs.cluster_endpoint
  cluster_ca_certificate = dependency.eks.outputs.cluster_certificate_authority_data
  
  # Configuration de l'App-of-Apps (le point d'entrée GitOps)
  repository_url = "git@github.com:ton-org/ton-repo.git"
  repository_path = "infrastructure/argocd-root"
  target_revision = "main"

  # On passe les ARN des rôles pour qu'ArgoCD puisse les injecter dans les YAML
  # via un mécanisme de "Values" ou de "ConfigMap"
  addons_iam_roles = dependency.identity.outputs.iam_role_arns
}