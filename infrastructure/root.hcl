# infrastructure/root.hcl

# ---------------------------------------------------------------------------------------------------------------------
# CONFIGURATION GLOBALE TERRAGRUNT
# Centralise la gestion du State S3, du verrouillage DynamoDB et des Providers.
# ---------------------------------------------------------------------------------------------------------------------

locals {
  # Charge automatiquement les variables communes selon l'emplacement du dossier (dev/prod)
  region_vars      = read_terragrunt_config(find_in_parent_folders("region.hcl", "empty.hcl"), { locals = { aws_region = "us-east-1" } })
  environment_vars = read_terragrunt_config(find_in_parent_folders("env.hcl", "empty.hcl"), { locals = { env = "dev" } })

  # Variables par défaut
  aws_region   = local.region_vars.locals.aws_region
  owner = "no-team"
  project_name = "core-platform"
}

# 1. GÉNÉRATION DU BACKEND (S3 + DynamoDB)
# Crée automatiquement le bucket et la table de lock si nécessaire
remote_state {
  backend = "s3"
  generate = {
    path      = "backend.tf"
    if_exists = "overwrite_terragrunt"
  }
  config = {
    bucket         = "${local.project_name}-terraform-state-${local.environment_vars.locals.env}"
    key            = "${path_relative_to_include()}/terraform.tfstate"
    region         = local.aws_region
    encrypt        = true
    use_lockfile   = true

    s3_bucket_tags = {
      Project = local.project_name
      Owner   = "infrastructure-team"
    }
  }
}

# 2. GÉNÉRATION DU PROVIDER AWS
# Injecte la configuration du provider dans tous les modules fils
generate "provider" {
  path      = "provider.tf"
  if_exists = "overwrite_terragrunt"
  contents  = <<EOF
provider "aws" {
  region = "${local.aws_region}"

  # Default tags appliqués à TOUTES les ressources du projet
  default_tags {
    tags = {
      Project     = "${local.project_name}"
      Environment = "${local.environment_vars.locals.env}"
      ManagedBy   = "Terraform/Terragrunt"
    }
  }
}
EOF
}

# 3. INPUTS GLOBAUX
# Ces variables sont passées à tous les modules sans avoir à les redéfinir
inputs = {
  aws_region   = local.aws_region
  project_name = local.project_name
  env          = local.environment_vars.locals.env
}