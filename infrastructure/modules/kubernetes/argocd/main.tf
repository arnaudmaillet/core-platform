# infrastructure/modules/kubernetes/argocd/main.tf

# 1. Installation du serveur ArgoCD via Helm
module "server" {
  source = "./server"

  cluster_name           = var.cluster_name
  cluster_endpoint       = var.cluster_endpoint
  cluster_ca_certificate = var.cluster_ca_certificate
  argocd_version         = var.argocd_version
}

# 2. Déploiement de l'Application Root (Bootstrap)
# Ce module ne se lance QUE lorsque le serveur est "Ready"
module "bootstrap" {
  source     = "./bootstrap"
  depends_on = [module.server] 

  cluster_name           = var.cluster_name
  cluster_endpoint       = var.cluster_endpoint
  cluster_ca_certificate = var.cluster_ca_certificate
  
  repository_url         = var.repository_url
  repository_path        = var.repository_path
  target_revision        = var.target_revision
  
  addons_iam_roles       = var.addons_iam_roles
  ssl_certificate_arn    = var.ssl_certificate_arn
}

module "addons" {
  source       = "./addons"
  cluster_name = var.cluster_name
  addons       = var.addons
}