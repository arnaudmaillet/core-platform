# infrastructure/modules/kubernetes/argocd/bootstrap/outputs.tf
output "root_app_name" {
  description = "Nom de l'application racine créée"
  value       = "root-bootstrap"
}