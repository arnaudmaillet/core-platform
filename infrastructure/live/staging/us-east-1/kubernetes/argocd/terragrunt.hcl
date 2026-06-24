# infrastructure/live/staging/us-east-1/kubernetes/argocd/terragrunt.hcl
#
# Staging's own per-cluster ArgoCD. Mirrors the dev component but points the
# root-bootstrap at the per-env appset tree (bootstrap/staging) and writes a
# per-env params file (global-params-staging.json) — so dev's bootstrap is
# untouched. Also enables the External Secrets Operator IRSA role.

include "root" {
  path = find_in_parent_folders("root.hcl")
}

locals {
  region_vars = read_terragrunt_config(find_in_parent_folders("region.hcl"))
  aws_region  = local.region_vars.locals.aws_region
}

dependency "vpc" {
  config_path = "../../networking/vpc"
}

dependency "eks" {
  config_path = "../../eks"
}

dependency "security" {
  config_path = "../../security/irsa-roles"
}

terraform {
  source = "../../../../../modules//kubernetes/argocd"
}

inputs = {
  region          = local.aws_region
  env             = "staging"
  argocd_version  = "7.7.0"
  repository_url  = "https://github.com/arnaudmaillet/core-platform"
  target_revision = "develop"

  # Per-env bootstrap wiring (defaults stay dev's; staging overrides).
  bootstrap_path     = "infrastructure/argocd/bootstrap/staging"
  global_params_file = "global-params-staging.json"

  # --- Cluster ---
  vpc_id                 = dependency.vpc.outputs.vpc_id
  cluster_name           = dependency.eks.outputs.cluster_name
  cluster_endpoint       = dependency.eks.outputs.cluster_endpoint
  cluster_ca_certificate = dependency.eks.outputs.cluster_certificate_authority_data

  # --- Security & Certificates ---
  ssl_certificate_arn = dependency.security.outputs.certificate_arn

  addons_iam_roles = {
    karpenter        = dependency.security.outputs.karpenter_role_arn
    lb_controller    = dependency.security.outputs.lb_controller_role_arn
    external_dns     = dependency.security.outputs.external_dns_role_arn
    cert_manager     = dependency.security.outputs.cert_manager_role_arn
    ebs_csi          = dependency.security.outputs.ebs_csi_role_arn
    external_secrets = dependency.security.outputs.external_secrets_role_arn
  }

  addons = {
    "aws-ebs-csi-driver" = {
      service_account_role_arn = dependency.security.outputs.ebs_csi_role_arn
    }
  }

  tags = {
    Project     = "core-platform"
    Environment = "staging"
    ManagedBy   = "Terraform/Terragrunt"
  }
}
