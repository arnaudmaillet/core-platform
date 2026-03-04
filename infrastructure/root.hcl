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

  repository_url = "https://github.com/arnaudmaillet/core-platform"
  repository_path = "infrastructure/argocd"
  target_revision = "develop"

  owner = "no-team"
  project_name = "core-platform"

  is_kubernetes = contains(split("/", path_relative_to_include()), "kubernetes")
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

# 2. GÉNÉRATION DES PROVIDERS (AWS, K8S, HELM, KUBECTL)
generate "providers" {
  path      = "providers.tf"
  if_exists = "overwrite_terragrunt"
  contents  = <<EOF
provider "aws" {
  region = "${local.aws_region}"
  default_tags {
    tags = {
      Project     = "${local.project_name}"
      Environment = "${local.environment_vars.locals.env}"
      ManagedBy   = "Terraform/Terragrunt"
    }
  }
}

provider "aws" {
  alias  = "virginia"
  region = "us-east-1"
}

%{ if local.is_kubernetes }
provider "kubernetes" {
  host                   = var.cluster_endpoint
  cluster_ca_certificate = base64decode(var.cluster_ca_certificate)
  exec {
    api_version = "client.authentication.k8s.io/v1beta1"
    command     = "aws"
    args        = ["eks", "get-token", "--cluster-name", var.cluster_name]
  }
}

provider "helm" {
  kubernetes = { # <--- ICI : Pour certaines versions, l'absence de '=' passe, mais pour la tienne il le faut
    host                   = var.cluster_endpoint
    cluster_ca_certificate = base64decode(var.cluster_ca_certificate)
    exec = {
      api_version = "client.authentication.k8s.io/v1beta1"
      command     = "aws"
      args        = ["eks", "get-token", "--cluster-name", var.cluster_name]
    }
  }
}

# Note : Si l'erreur persiste après avoir ajouté le '=', 
# c'est que ta version de Helm attend la syntaxe sans '=' mais avec un bloc nommé différemment.
# Mais l'erreur "Did you mean to define argument" confirme qu'il attend '='.

provider "kubectl" {
  host                   = var.cluster_endpoint
  cluster_ca_certificate = base64decode(var.cluster_ca_certificate)
  load_config_file       = false
  exec {
    api_version = "client.authentication.k8s.io/v1beta1"
    command     = "aws"
    args        = ["eks", "get-token", "--cluster-name", var.cluster_name]
  }
}
%{ endif }
EOF
}

generate "versions" {
  path      = "versions.tf"
  if_exists = "overwrite_terragrunt"
  contents  = <<EOF
terraform {
  required_version = ">= 1.0"
  required_providers {
    aws        = { source = "hashicorp/aws", version = ">= 5.0" }
%{ if local.is_kubernetes }
    kubernetes = { source = "hashicorp/kubernetes", version = ">= 2.20" }
    helm       = { source = "hashicorp/helm", version = ">= 2.10" }
    kubectl    = { source = "gavinbunney/kubectl", version = ">= 1.14.0" }
%{ endif }
    time       = { source = "hashicorp/time", version = ">= 0.9" }
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
  repository_url  = local.repository_url
  repository_path = local.repository_path
  target_revision = local.target_revision
}

# 4. AUTOMATISATION DU LOGIN HELM (ECR PUBLIC)
# Évite l'erreur 403 "authorization token has expired" toutes les 12h
terraform {
  before_hook "ecr_public_login" {
    commands = ["apply", "plan"]
    execute  = [
      "sh", 
      "-c", 
      "aws ecr-public get-login-password --region us-east-1 | helm registry login --username AWS --password-stdin public.ecr.aws"
    ]
  }
}