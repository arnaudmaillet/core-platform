# infrastructure/modules/services/microservices/workers/main.tf

resource "kubernetes_deployment" "this" {
  metadata {
    name      = var.name
    namespace = var.namespace
    labels    = { app = var.name, type = "worker" }
  }

  spec {
    replicas = var.replicas
    selector { match_labels = { app = var.name } }

    template {
      metadata { labels = { app = var.name } }
      spec {
        service_account_name = kubernetes_service_account.this.metadata[0].name
        container {
          name  = var.name
          image = var.image

          # Pas de port {} défini ici

          resources {
            limits   = { cpu = var.cpu_limit, memory = var.memory_limit }
            requests = { cpu = var.cpu_request, memory = var.memory_request }
          }

          dynamic "env" {
            for_each = var.env_vars
            content {
              name  = env.key
              value = env.value
            }
          }

          env {
            name = "DATABASE_URL"
            value_from {
              secret_key_ref {
                name = "db-credentials"
                key  = "DATABASE_URL"
              }
            }
          }

          # On garde uniquement la liveness_probe pour vérifier si le process Rust a crashé
          # Mais on ne peut pas utiliser tcp_socket sans port, donc on utilise souvent une commande exec
          # Ou on la retire si le binaire Rust gère son propre cycle de vie.
        }
      }
    }
  }
}

# --- Service Account & IAM (Identique à l'API pour garder l'accès DB) ---

resource "kubernetes_service_account" "this" {
  metadata {
    name      = var.name
    namespace = var.namespace
    annotations = { "eks.amazonaws.com/role-arn" = module.iam_eks_role.iam_role_arn }
  }
}

resource "kubernetes_manifest" "external_secret" {
  manifest = {
    apiVersion = "external-secrets.io/v1"
    kind       = "ExternalSecret"
    metadata = {
      name      = "${var.name}-db-secret"
      namespace = var.namespace
    }
    spec = {
      refreshInterval = "1h"
      secretStoreRef  = { name = "aws-secretsmanager", kind = "ClusterSecretStore" }
      target          = { name = "db-credentials" }
      data = [{
        secretKey = "DATABASE_URL"
        remoteRef = { key = "${var.project_name}/${var.env}/${var.name}/db-url" }
      }]
    }
  }
}

module "iam_eks_role" {
  source    = "terraform-aws-modules/iam/aws//modules/iam-role-for-service-accounts-eks"
  version   = "~> 5.0"
  role_name = "${var.project_name}-${var.env}-${var.name}-irsa"
  oidc_providers = {
    main = {
      provider_arn               = var.oidc_provider_arn
      namespace_service_accounts = ["${var.namespace}:${var.name}"]
    }
  }
  role_policy_arns = { policy = aws_iam_policy.secrets_policy.arn }
}

resource "aws_iam_policy" "secrets_policy" {
  name   = "${var.project_name}-${var.env}-${var.name}-secrets-policy"
  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Action   = "secretsmanager:GetSecretValue"
      Effect   = "Allow"
      Resource = var.db_secret_arn
    }]
  })
}