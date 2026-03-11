# infrastructure/modules/kubernetes/argocd/outputs.tf

# infrastructure/modules/kubernetes/argocd/outputs.tf

output "argocd_namespace" {
  value = module.server.namespace
}