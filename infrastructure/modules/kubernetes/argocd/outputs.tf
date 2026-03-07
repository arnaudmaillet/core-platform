# infrastructure/modules/kubernetes/argocd/outputs.tf

output "argocd_namespace" {
  value = module.server.namespace
}

output "root_app_name" {
  value = module.bootstrap.root_app_name
}