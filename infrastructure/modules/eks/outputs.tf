# infrastructure/modules/eks/outputs.tf

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
# Ces outputs sont vitaux pour le module Identity
output "oidc_provider_arn" {
  description = "L'ARN du provider OIDC pour les rôles IAM des Service Accounts (IRSA)"
  value       = module.eks.oidc_provider_arn
}

output "oidc_provider_url" {
  description = "L'URL de l'OIDC Provider"
  value       = module.eks.cluster_oidc_issuer_url
}

# --- NOEUDS & RÉSEAU ---
output "node_iam_role_arns" {
  description = "Liste des ARNs des rôles IAM des Managed Node Groups (pour le PassRole de Karpenter)"
  value       = [for group in module.eks.eks_managed_node_groups : group.iam_role_arn]
}