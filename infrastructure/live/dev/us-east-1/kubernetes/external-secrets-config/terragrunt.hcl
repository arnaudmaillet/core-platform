# infrastructure/live/dev/us-east-1/kubernetes/external-secrets-config/terragrunt.hcl

include "root" {
  path = find_in_parent_folders("root.hcl")
}

# On ne définit PAS de bloc terraform { source = ... }
# car on génère tout nous-mêmes ci-dessous.
# Terragrunt utilisera un dossier local vide par défaut.

dependency "eks" {
  config_path = "../eks"
}

dependency "operator" {
  config_path = "../external-secrets"
  skip_outputs = true
}

# 1. Manifeste du Store (garde ton code actuel)
generate "manifests" {
  path      = "manifests.tf"
  if_exists = "overwrite_terragrunt"
  contents  = <<EOF
resource "kubernetes_manifest" "cluster_secret_store" {
  manifest = {
    apiVersion = "external-secrets.io/v1"
    kind       = "ClusterSecretStore"
    metadata = {
      name = "aws-secretsmanager"
    }
    spec = {
      provider = {
        aws = {
          service = "SecretsManager"
          region  = "us-east-1"
        }
      }
    }
  }
}
EOF
}

# 2. Provider Kubernetes (garde ton code actuel)
generate "provider_kubernetes_config" {
  path      = "provider_k8s.tf"
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