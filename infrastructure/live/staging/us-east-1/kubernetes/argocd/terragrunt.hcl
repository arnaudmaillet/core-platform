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

# Data-store endpoints fed into the repo-server envsubst CMP (cmp-envsubst-values
# Secret) so the workload overlay's ${VAR} endpoints resolve. mock_outputs let
# `plan`/`validate` run before the data stores exist (e.g. first run-all).
dependency "msk" {
  config_path                             = "../../data/msk"
  mock_outputs                            = { bootstrap_brokers_sasl_scram = "b-mock:9096" }
  mock_outputs_allowed_terraform_commands = ["validate", "plan"]
}

dependency "elasticache" {
  config_path                             = "../../data/elasticache"
  mock_outputs                            = { configuration_endpoint = "mock.cache.amazonaws.com" }
  mock_outputs_allowed_terraform_commands = ["validate", "plan"]
}

dependency "opensearch" {
  config_path                             = "../../data/opensearch"
  mock_outputs                            = { endpoint = "mock.es.amazonaws.com" }
  mock_outputs_allowed_terraform_commands = ["validate", "plan"]
}

# Ordering-only: the workload ExternalSecrets (synced by ArgoCD) pull the app
# secrets this unit seeds, so it must apply before ArgoCD brings up the fleet.
# No outputs consumed here.
dependency "app_secrets" {
  config_path  = "../../data/app-secrets"
  skip_outputs = true
}

terraform {
  source = "../../../../../modules//kubernetes/argocd"

  # Drain operator-managed AWS resources (ALBs/NLBs, CNPG/Scylla EBS, Karpenter
  # EC2) from inside the live cluster BEFORE Terraform deletes the cluster/VPC —
  # otherwise they leak and leftover LB ENIs block VPC destroy. Staging is the
  # one that actually carries the public NLB + 6 CNPG clusters + Scylla, so this
  # is load-bearing here. Shared script keeps dev and staging in lockstep.
  before_hook "graceful_cleanup" {
    commands = ["destroy"]
    execute = [
      "/bin/bash",
      "${get_repo_root()}/infrastructure/assets/teardown/k8s-graceful-cleanup.sh",
      dependency.eks.outputs.cluster_name,
      local.aws_region,
    ]
  }
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

  # --- CMP envsubst values (data-store endpoints for the workload overlay) ---
  msk_bootstrap_brokers = dependency.msk.outputs.bootstrap_brokers_sasl_scram
  elasticache_endpoint  = dependency.elasticache.outputs.configuration_endpoint
  opensearch_endpoint   = dependency.opensearch.outputs.endpoint
  # auth_jwks_url / keycloak_token_endpoint stay empty until auth/Keycloak lands.

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
