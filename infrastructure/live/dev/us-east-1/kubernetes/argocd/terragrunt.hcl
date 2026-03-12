# infrastructure/live/dev/us-east-1/kubernetes/argocd/terragrunt.hcl

include "root" {
  path = find_in_parent_folders("root.hcl")
}

dependency "eks" {
  config_path = "../../eks"
}

dependency "security" {
  config_path = "../../security/irsa-roles" 
}

terraform {
  source = "../../../../../modules//kubernetes/argocd"

  before_hook "clean_k8s_resources" {
    commands     = ["destroy"]
    execute      = ["/bin/bash", "-c", "kubectl delete ingress --all --all-namespaces --wait=true || true"]
  }
}

inputs = {
  # --- Paramètres du Cluster ---
  cluster_name           = dependency.eks.outputs.cluster_name
  cluster_endpoint       = dependency.eks.outputs.cluster_endpoint
  cluster_ca_certificate = dependency.eks.outputs.cluster_certificate_authority_data

  # --- Paramètres ArgoCD (Nouveau !) ---
  argocd_version  = "7.7.0"
  repository_url  = "https://github.com/arnaudmaillet/core-platform"
  target_revision = "env/dev"
  env             = "dev"

  # --- Sécurité & Certificats ---
  ssl_certificate_arn = dependency.security.outputs.certificate_arn
  
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
      addon_version            = "v1.28.0-eksbuild.1" # Optionnel, ou (known after apply)
      service_account_role_arn = dependency.security.outputs.ebs_csi_role_arn
    }
  }

  tags = {
    Project     = "core-platform"
    Environment = "dev"
    ManagedBy   = "Terraform/Terragrunt"
  }
}