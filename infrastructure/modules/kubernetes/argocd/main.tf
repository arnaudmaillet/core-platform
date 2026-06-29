# infrastructure/modules/kubernetes/argocd/main.tf

provider "github" {
  owner = "arnaudmaillet"
}

# 1. Installation du serveur ArgoCD + Auto-Bootstrap
# Le serveur s'installe ET déploie l'application racine en une seule étape atomique.
module "server" {
  source                 = "./server"
  cluster_name           = var.cluster_name
  cluster_endpoint       = var.cluster_endpoint
  cluster_ca_certificate = var.cluster_ca_certificate
  argocd_version         = var.argocd_version
}

module "bootstrap" {
  source     = "./bootstrap"
  depends_on = [module.server]

  env                 = var.env
  region              = var.region
  repository_url      = var.repository_url
  target_revision     = var.target_revision
  cluster_name        = var.cluster_name
  cluster_endpoint    = var.cluster_endpoint
  vpc_id              = var.vpc_id
  ssl_certificate_arn = var.ssl_certificate_arn
  addons_iam_roles    = var.addons_iam_roles
  bootstrap_path      = var.bootstrap_path
  global_params_file  = var.global_params_file
  server_dependency   = module.server.argocd_id

  # CMP envsubst values (data-store endpoints) for the workload overlay render.
  msk_bootstrap_brokers   = var.msk_bootstrap_brokers
  elasticache_endpoint    = var.elasticache_endpoint
  opensearch_endpoint     = var.opensearch_endpoint
  auth_jwks_url           = var.auth_jwks_url
  keycloak_token_endpoint = var.keycloak_token_endpoint
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
