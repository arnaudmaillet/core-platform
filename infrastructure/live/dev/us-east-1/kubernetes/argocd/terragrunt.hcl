# infrastructure/live/dev/us-east-1/kubernetes/argocd/terragrunt.hcl

include "root" {
  path = find_in_parent_folders("root.hcl")
}

# --- DÉPENDANCES ---
dependency "eks" {
  config_path = "../../eks"
}

dependency "security" {
  # On pointe vers ton nouveau dossier de sécurité sans numéro
  config_path = "../../security/irsa-roles" 
}

terraform {
  # On pointe vers le nouveau module parent "argocd" qui contient server, bootstrap et addons
  source = "../../../../../modules//kubernetes/argocd"

  # On garde le hook de nettoyage sur le module global
  before_hook "clean_k8s_resources" {
    commands     = ["destroy"]
    execute      = ["/bin/bash", "-c", "kubectl delete ingress --all --all-namespaces --wait=true || true"]
  }
}

# --- INPUTS UNIFIÉS ---
inputs = {
  # Infos Cluster
  cluster_name           = dependency.eks.outputs.cluster_name
  cluster_endpoint       = dependency.eks.outputs.cluster_endpoint
  cluster_ca_certificate = dependency.eks.outputs.cluster_certificate_authority_data

  # Sécurité & Certificats
  ssl_certificate_arn    = dependency.security.outputs.certificate_arn
  
  # Rôles IAM (on passe la map complète attendue par ton bootstrap)
  addons_iam_roles = {
    karpenter     = dependency.security.outputs.karpenter_role_arn
    lb_controller = dependency.security.outputs.lb_controller_role_arn
    external_dns  = dependency.security.outputs.external_dns_role_arn
    cert_manager  = dependency.security.outputs.cert_manager_role_arn
    ebs_csi       = dependency.security.outputs.ebs_csi_role_arn
  }
  # Addons EKS (Gestion AWS)
  addons = {
    aws-ebs-csi-driver = {
      service_account_role_arn = dependency.security.outputs.ebs_csi_role_arn
    }
  }

  tags = {
    Environment = "dev"
    ManagedBy   = "terragrunt"
  }
}