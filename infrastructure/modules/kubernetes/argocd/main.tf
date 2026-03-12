# infrastructure/modules/kubernetes/argocd/main.tf

# 1. Installation du serveur ArgoCD + Auto-Bootstrap
# Le serveur s'installe ET déploie l'application racine en une seule étape atomique.
module "server" {
  source = "./server"
  cluster_name           = var.cluster_name
  cluster_endpoint       = var.cluster_endpoint
  cluster_ca_certificate = var.cluster_ca_certificate
  argocd_version         = var.argocd_version
}

module "bootstrap" {
  source = "./bootstrap"
  repository_url  = var.repository_url
  target_revision = var.target_revision

  depends_on = [module.server]
}

# 2. Installation des Addons EKS
# On dépend maintenant directement du serveur. 
# Dès que le serveur est "Ready", les addons peuvent être provisionnés.
module "addons" {
  source       = "./addons"
  depends_on   = [module.server]
  cluster_name = var.cluster_name
  addons       = var.addons
}