# infrastructure/live/dev/us-east-1/networking/terragrunt.hcl

# Indique à Terragrunt d'inclure la configuration racine (S3, Providers, etc.)
include "root" {
  path   = find_in_parent_folders("root.hcl")
  expose = true
}

# Indique où se trouve le code source du module (le blueprint)
terraform {
  source = "${get_repo_root()}//infrastructure/modules/networking"
}

# On passe les variables spécifiques à ce déploiement "dev"
inputs = {
  cluster_name       = "core-eks-dev"
  vpc_cidr           = "10.0.0.0/16"
  availability_zones = ["us-east-1a", "us-east-1b", "us-east-1c"]
}