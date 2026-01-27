# infrastructure/modules/services/microservices/outputs.tf

output "service_name" {
  value = kubernetes_service.this.metadata[0].name
}

output "service_port" {
  value = kubernetes_service.this.spec[0].port[0].port
}

output "service_account_name" {
  description = "Le nom du Service Account utilisé"
  value       = kubernetes_service_account.this.metadata[0].name
}

output "iam_role_arn" {
  description = "L'ARN du rôle IAM AWS créé pour ce service"
  value       = module.iam_eks_role.iam_role_arn
}

output "external_secret_name" {
  description = "Le nom de l'ExternalSecret créé"
  value       = kubernetes_manifest.external_secret.manifest.metadata.name
}