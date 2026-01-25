# Indique à Terragrunt d'inclure la configuration racine (S3, Providers, etc.)
include "root" {
  path   = find_in_parent_folders("root.hcl")
  expose = true
}

# Indique où se trouve le code source du module (le blueprint)
terraform {
  source = "../../../../modules/networking"
}

# On passe les variables spécifiques à ce déploiement "dev"
inputs = {
  project_name       = "core-platform"
  env                = "dev"
  cluster_name       = "core-eks-dev"
  vpc_cidr           = "10.0.0.0/16"
  availability_zones = ["us-east-1a", "us-east-1b", "us-east-1c"]
}