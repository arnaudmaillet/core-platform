# infrastructure/modules/kubernetes/argocd/outputs.tv

output "argocd_namespace" {
  value = module.server.namespace
}