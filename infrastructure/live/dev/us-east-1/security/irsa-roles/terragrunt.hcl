# infrastructure/live/dev/us-east-1/kubernetes/00-identity/terragrunt.hcl

include "root" {
  path = find_in_parent_folders("root.hcl")
}

dependency "eks" {
  config_path = "../../eks"
}

terraform {
  source = "../../../../../modules//security/irsa-roles"
}

inputs = {
  cluster_name      = dependency.eks.outputs.cluster_name
  oidc_provider_arn = dependency.eks.outputs.oidc_provider_arn
  oidc_provider_url = dependency.eks.outputs.oidc_provider_url


  # Crucial pour Karpenter : on lui dit quels rôles de nodes il a le droit d'utiliser
  node_iam_role_arns = dependency.eks.outputs.node_iam_role_arns

  # Centralisation de la policy JSON (standard SRE)
  iam_policy_json_content = file("${get_repo_root()}/infrastructure/assets/iam-policies/aws-lb-controller.json")

  tags = {
    Environment = "dev"
    ManagedBy   = "terragrunt"
  }
}