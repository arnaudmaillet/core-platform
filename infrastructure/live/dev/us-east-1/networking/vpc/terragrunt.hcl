# infrastructure/live/dev/us-east-1/networking/vpc/terragrunt.hcl

# Indique à Terragrunt d'inclure la configuration racine (S3, Providers, etc.)
include "root" {
  path   = find_in_parent_folders("root.hcl")
  expose = true
}

# Indique où se trouve le code source du module (le blueprint)
terraform {
  source = "../../../../../modules/networking/vpc"
}

locals {
  # On récupère les variables centralisées
  env_vars    = read_terragrunt_config(find_in_parent_folders("env.hcl"))
  region_vars = read_terragrunt_config(find_in_parent_folders("region.hcl"))
}

# On passe les variables spécifiques à ce déploiement "dev"
inputs = {
  cluster_name       = "core-platform-${local.env_vars.locals.env}"
  vpc_cidr           = local.env_vars.locals.vpc_cidr
  availability_zones = ["${local.region_vars.locals.aws_region}a", "${local.region_vars.locals.aws_region}b"]
}