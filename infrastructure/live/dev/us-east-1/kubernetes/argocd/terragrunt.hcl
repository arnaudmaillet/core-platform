# infrastructure/live/dev/us-east-1/kubernetes/argocd/terragrunt.hcl

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

# ACM cert (decoupled from eks so its teardown can't leak the VPC — see the unit).
dependency "acm_cert" {
  config_path                             = "../../networking/acm-cert"
  mock_outputs                            = { certificate_arn = "arn:aws:acm:us-east-1:000000000000:certificate/mock" }
  mock_outputs_allowed_terraform_commands = ["validate", "plan"]
}

terraform {
  source = "../../../../../modules//kubernetes/argocd"

  # Drain operator-managed AWS resources (ALBs/NLBs, CNPG/Scylla EBS, Karpenter
  # EC2) from inside the live cluster BEFORE Terraform deletes the cluster/VPC —
  # otherwise they leak and leftover LB ENIs block VPC destroy. Shared script so
  # dev and staging stay in lockstep. See the script header for the rationale.
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
  env             = "dev"
  # argo-cd chart 10.9.1 = Argo CD v3.5.3, tested upstream on Kubernetes 1.33–1.36.
  # (7.7.0 = v2.13.0 was tested up to 1.31 and cannot diff Deployments on 1.33+:
  # "structured merge diff ... .status.terminatingReplicas" — found live 2026-09-15.)
  # The CMP sidecar image is derived from this pin in modules/kubernetes/argocd/server.
  argocd_version  = "10.9.1"
  repository_url  = "https://github.com/arnaudmaillet/core-platform"
  target_revision = "develop"

  # --- Paramètres du Cluster ---
  vpc_id                 = dependency.vpc.outputs.vpc_id
  cluster_name           = dependency.eks.outputs.cluster_name
  cluster_endpoint       = dependency.eks.outputs.cluster_endpoint
  cluster_ca_certificate = dependency.eks.outputs.cluster_certificate_authority_data


  # --- Sécurité & Certificats ---
  ssl_certificate_arn = dependency.acm_cert.outputs.certificate_arn

  addons_iam_roles = {
    karpenter     = dependency.security.outputs.karpenter_role_arn
    lb_controller = dependency.security.outputs.lb_controller_role_arn
    external_dns  = dependency.security.outputs.external_dns_role_arn
    cert_manager  = dependency.security.outputs.cert_manager_role_arn
    ebs_csi       = dependency.security.outputs.ebs_csi_role_arn
  }

  # --- Addons EKS (Gérés par le module addons) ---
  addons = {
    "aws-ebs-csi-driver" = {
      service_account_role_arn = dependency.security.outputs.ebs_csi_role_arn
    }
  }

  tags = {
    Project     = "core-platform"
    Environment = "dev"
    ManagedBy   = "Terraform/Terragrunt"
  }
}