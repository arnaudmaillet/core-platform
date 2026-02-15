# infrastructure/modules/kubernetes/eks/outputs.tf

output "cluster_endpoint" {
  value = module.eks.cluster_endpoint
}

output "cluster_name" {
  value = module.eks.cluster_name
}

output "node_security_group_id" {
  description = "ID du Security Group des workers EKS"
  value       = module.eks.node_security_group_id
}

output "cluster_certificate_authority_data" {
  value = module.eks.cluster_certificate_authority_data
}

output "karpenter_node_role_name" {
  value = module.karpenter.node_iam_role_name
}

output "oidc_provider_arn" {
  description = "L'ARN du provider OIDC pour les rôles IAM des Service Accounts (IRSA)"
  value       = module.eks.oidc_provider_arn
}

output "oidc_provider" {
  description = "L'URL de l'OIDC Provider (sans le protocole https://)"
  value       = replace(module.eks.cluster_oidc_issuer_url, "https://", "")
}