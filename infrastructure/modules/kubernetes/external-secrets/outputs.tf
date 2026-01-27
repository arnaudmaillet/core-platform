# infrastructure/modules/kubernetes/external-secret/outputs.tf

output "external_secrets_role_arn" {
  description = "L'ARN du rôle IAM utilisé par l'opérateur External Secrets"
  value       = module.external_secrets_irsa.iam_role_arn
}

output "helm_release_status" {
  description = "Statut du déploiement Helm"
  value       = helm_release.external_secrets.status
}