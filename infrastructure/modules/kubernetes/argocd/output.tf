# infrastructure/modules/kubernetes/argocd/output.tf

output "argocd_namespace" {
  value = helm_release.argocd.namespace
}

output "root_app_name" {
  value = kubernetes_manifest.root_application.manifest.metadata.name
}