# infrastructure/live/dev/us-east-1/services/microservices/profile-service/terragrunt.hcl

include "root" {
  path = find_in_parent_folders("root.hcl")
}

terraform {
  source = "${get_repo_root()}//infrastructure/modules/services/api"
}

# 1. On récupère les données du cluster
dependency "eks" {
  config_path = "../../../../kubernetes/eks"
}

dependency "db" {
  config_path = "../../../../data/postgres"
}

# 2. ON INJECTE LE CODE DU PROVIDER (Crucial pour éviter l'erreur localhost)
generate "provider_kubernetes" {
  path      = "provider_kubernetes.tf"
  if_exists = "overwrite_terragrunt"
  contents  = <<EOF
provider "kubernetes" {
  host                   = "${dependency.eks.outputs.cluster_endpoint}"
  cluster_ca_certificate = base64decode("${dependency.eks.outputs.cluster_certificate_authority_data}")
  exec {
    api_version = "client.authentication.k8s.io/v1beta1"
    args        = ["eks", "get-token", "--cluster-name", "${dependency.eks.outputs.cluster_name}"]
    command     = "aws"
  }
}
EOF
}

# 3. Paramètres du service
inputs = {
  name               = "profile-service"
  namespace          = "default"

  # Injecté dans le rôle IAM du microservice
  oidc_provider_arn  = dependency.eks.outputs.oidc_provider_arn
  db_secret_arn      = dependency.db.outputs.db_secret_arn

  image    = "724772065879.dkr.ecr.us-east-1.amazonaws.com/core-platform-backend:profile_command_server"
  port     = 50051
  replicas = 1

  env_vars = {
    RUST_LOG = "info"
    # Note : DATABASE_URL ne sera PLUS ici en dur.
    # Elle sera injectée par External Secrets Operator.
  }
}