# infrastructure/live/dev/us-east-1/kubernetes/external-secrets-config/terragrunt.hcl

include "root" {
  path = find_in_parent_folders("root.hcl")
}

# On définit EKS avec des mocks pour que le plan ne plante pas
dependency "eks" {
  config_path = "../eks"

  mock_outputs = {
    cluster_endpoint              = "https://mock-endpoint.eks.amazonaws.com"
    cluster_certificate_authority_data = "bW9jay1jYQ==" # 'mock-ca' en base64
    cluster_name                  = "mock-cluster"
  }
}

# L'opérateur n'a pas d'outputs obligatoires, on l'utilise surtout pour l'ordre
dependency "operator" {
  config_path = "../external-secrets"

  skip_outputs = true # Comme on n'utilise pas ses outputs, on évite les erreurs
}

# 1. Manifeste du Store
generate "manifests" {
  path      = "manifests.tf"
  if_exists = "overwrite_terragrunt"
  contents  = <<EOF
resource "kubernetes_manifest" "cluster_secret_store" {
  manifest = {
    # CHANGEMENT ICI : v1 au lieu de v1beta1
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
          auth = {
            jwt = {
              serviceAccountRef = {
                name      = "external-secrets"
                namespace = "external-secrets"
              }
            }
          }
        }
      }
    }
  }
}
EOF
}

# 2. Provider Kubernetes avec les variables de dépendance
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