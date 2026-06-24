# infrastructure/live/staging/us-east-1/security/irsa-roles/terragrunt.hcl
# Mirrors the dev IRSA component, plus the External Secrets Operator role
# (enable_external_secrets) so the staging app can read MSK/Redis creds from
# Secrets Manager.

include "root" {
  path = find_in_parent_folders("root.hcl")
}

dependency "eks" {
  config_path = "../../eks"
}

terraform {
  source = "../../../../../modules/security/irsa-roles"
}

inputs = {
  cluster_name       = dependency.eks.outputs.cluster_name
  oidc_provider_arn  = dependency.eks.outputs.oidc_provider_arn
  oidc_provider_url  = dependency.eks.outputs.oidc_provider_url
  node_iam_role_arns = dependency.eks.outputs.node_iam_role_arns

  iam_policy_json_content = file("${get_repo_root()}/infrastructure/assets/iam-policies/aws-lb-controller.json")

  # Staging consumes managed backends -> ESO reads their Secrets Manager creds.
  enable_external_secrets = true

  tags = {
    Environment = "staging"
    ManagedBy   = "terragrunt"
  }
}
