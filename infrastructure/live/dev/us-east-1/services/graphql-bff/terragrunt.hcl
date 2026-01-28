# infrastructure/live/dev/us-east-1/services/microservices/graphql-bff/terragrunt.hcl

include "root" {
  path = find_in_parent_folders("root.hcl")
}

terraform {
  source = "${get_repo_root()}//infrastructure/modules/services/microservices"
}

# 1. Dépendance vers le cluster EKS pour récupérer les accès
dependency "eks" {
  config_path = "../../kubernetes"
}

# 2. Génération dynamique du provider Kubernetes
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

# 3. Paramètres de l'application
inputs = {
  name     = "graphql-bff"
  image    = "724772065879.dkr.ecr.us-east-1.amazonaws.com/core-platform-backend:graphql_bff"
  port     = 50051
  replicas = 2

  env_vars = {
    PROFILE_SERVICE_URL = "http://profile-service:50051"
    RUST_LOG            = "info"
  }

  # Pour éviter que Terragrunt ne freeze pendant 15min si le pod ne passe pas Ready
  wait_for_rollout = false
}