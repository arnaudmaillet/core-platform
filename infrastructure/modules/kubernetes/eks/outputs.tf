# infrastructure/modules/kubernetes/eks/outputs.tf

# --- INFRASTRUCTURE CLUSTER ---

output "cluster_name" {
  description = "Le nom du cluster EKS"
  value       = module.eks.cluster_name
}

output "cluster_endpoint" {
  description = "L'URL de l'API server Kubernetes"
  value       = module.eks.cluster_endpoint
}

output "cluster_certificate_authority_data" {
  description = "Certificat CA du cluster pour la configuration des clients (Helm/Kubectl)"
  value       = module.eks.cluster_certificate_authority_data
}

output "node_security_group_id" {
  description = "ID du Security Group des workers EKS (utile pour Karpenter)"
  value       = module.eks.node_security_group_id
}

# --- IDENTITÉ (IAM & OIDC) ---

output "oidc_provider_arn" {
  description = "L'ARN du provider OIDC pour les rôles IAM des Service Accounts (IRSA)"
  value       = module.eks.oidc_provider_arn
}

output "oidc_provider" {
  description = "L'URL de l'OIDC Provider (utilisé par certains contrôleurs)"
  value       = replace(module.eks.cluster_oidc_issuer_url, "https://", "")
}

# --- ARNs DES RÔLES IAM (Crucial pour le module Addons) ---

output "lb_controller_role_arn" {
  description = "ARN du rôle IAM pour AWS Load Balancer Controller"
  value       = module.lb_controller_irsa_role.iam_role_arn
}

output "ebs_csi_role_arn" {
  description = "ARN du rôle IAM pour le driver EBS CSI"
  value       = module.ebs_csi_irsa_role.iam_role_arn
}

output "external_dns_role_arn" {
  description = "ARN du rôle IAM pour External-DNS"
  value       = module.external_dns_irsa_role.iam_role_arn
}

# --- KARPENTER & RÉSEAU ---

output "karpenter_node_role_name" {
  description = "Nom du rôle IAM utilisé par les futurs nœuds créés par Karpenter"
  value = module.eks.eks_managed_node_groups["system"].iam_role_name
}

output "karpenter_controller_role_arn" {
  description = "ARN du rôle IAM (IRSA) pour le contrôleur Karpenter"
  value       = module.karpenter_irsa_role.iam_role_arn
}

output "ssl_certificate_arn" {
  description = "L'ARN du certificat ACM validé pour l'Ingress"
  value       = aws_acm_certificate.cert.arn
}