# infrastructure/modules/kubernetes/argocd/bootstrap/outputs.tf

output "root_app_name" {
  value = kubernetes_manifest.root_application.manifest.metadata.name
}