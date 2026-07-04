# infrastructure/modules/kubernetes/argocd/server/outputs.tf

output "namespace" {
  description = "Le namespace où ArgoCD est installé"
  value       = helm_release.argocd.namespace
}

output "chart_version" {
  description = "La version du chart installée"
  value       = helm_release.argocd.version
}

output "argocd_id" {
  value = helm_release.argocd.id
}