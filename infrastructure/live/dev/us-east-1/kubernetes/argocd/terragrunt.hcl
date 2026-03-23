# infrastructure/live/dev/us-east-1/kubernetes/argocd/terragrunt.hcl

include "root" {
  path = find_in_parent_folders("root.hcl")
}

locals {
  region_vars = read_terragrunt_config(find_in_parent_folders("region.hcl"))
  aws_region = local.region_vars.locals.aws_region
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

  before_hook "graceful_cleanup" {
    commands     = ["destroy"]
    execute      = ["/bin/bash", "-c", <<-EOT
      echo "--- Graceful Cleanup Start ---"
      # 1. Désactiver le self-heal sur la root-app pour éviter les recréations
      kubectl patch app root-bootstrap -n argocd --type merge -p '{"spec":{"syncPolicy":null}}' || true
      
      # 2. Supprimer les Ingress de tout le cluster (pour libérer les Load Balancers AWS)
      # On attend qu'ils soient vraiment supprimés côté AWS
      kubectl delete ingress --all -A --timeout=90s || echo "Ingress deletion timed out, continuing..."
      
      # 3. Supprimer les ApplicationSets (pour arrêter de générer de nouvelles apps)
      kubectl delete appsets --all -A || true
      
      # 4. Supprimer les Apps enfants avec cascade pour nettoyer les ressources cloud
      kubectl delete app -n argocd -l argocd.argoproj.io/instance!=root-bootstrap --cascade=foreground --timeout=120s || true
      
      echo "--- Cleanup finished, proceeding to Terraform Destroy ---"
    EOT
    ]
  }
}

inputs = {
  region          = local.aws_region
  env             = "dev"
  argocd_version  = "7.7.0"
  repository_url  = "https://github.com/arnaudmaillet/core-platform"
  target_revision = "develop"

  # --- Paramètres du Cluster ---
  vpc_id                 = dependency.vpc.outputs.vpc_id
  cluster_name           = dependency.eks.outputs.cluster_name
  cluster_endpoint       = dependency.eks.outputs.cluster_endpoint
  cluster_ca_certificate = dependency.eks.outputs.cluster_certificate_authority_data


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
      service_account_role_arn = dependency.security.outputs.ebs_csi_role_arn
    }
  }

  tags = {
    Project     = "core-platform"
    Environment = "dev"
    ManagedBy   = "Terraform/Terragrunt"
  }
}