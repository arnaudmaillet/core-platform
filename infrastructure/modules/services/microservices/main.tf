# infrastructure/modules/services/microservices/main.tf

resource "kubernetes_deployment" "this" {
  metadata {
    name      = var.name
    namespace = var.namespace
    labels = {
      app = var.name
    }
  }

  spec {
    replicas = var.replicas

    selector {
      match_labels = {
        app = var.name
      }
    }

    template {
      metadata {
        labels = {
          app = var.name
        }
      }

      spec {
        service_account_name = kubernetes_service_account.this.metadata[0].name

        container {
          name  = var.name
          image = var.image

          port {
            container_port = var.port
          }

          # Gestion des ressources pour l'autoscaling (Karpenter/HPA)
          resources {
            limits = {
              cpu    = var.cpu_limit
              memory = var.memory_limit
            }
            requests = {
              cpu    = var.cpu_request
              memory = var.memory_request
            }
          }

          # Injection dynamique des variables d'environnement
          dynamic "env" {
            for_each = var.env_vars
            content {
              name  = env.key
              value = env.value
            }
          }

          # Injection de la DATABASE_URL depuis le secret
          env {
            name = "DATABASE_URL"
            value_from {
              secret_key_ref {
                name = "db-credentials" # Doit matcher le target.name de l'ExternalSecret
                key  = "DATABASE_URL"
              }
            }
          }

          # Liveness Probe : Kubernetes redémarre le pod s'il crash
          liveness_probe {
            tcp_socket {
              port = var.port
            }
            initial_delay_seconds = 5
            period_seconds        = 10
          }

          # Readiness Probe : Kubernetes n'envoie du trafic que si le pod est prêt
          readiness_probe {
            tcp_socket {
              port = var.port
            }
            initial_delay_seconds = 5
            period_seconds        = 10
          }
        }
      }
    }
  }
}

resource "kubernetes_service" "this" {
  metadata {
    name      = var.name
    namespace = var.namespace
  }

  spec {
    selector = {
      app = var.name
    }

    port {
      port        = var.port
      target_port = var.port
      protocol    = "TCP"
    }

    type = "ClusterIP"
  }
}

# On crée l'identité Kubernetes pour le Pod
resource "kubernetes_service_account" "this" {
  metadata {
    name      = var.name
    namespace = var.namespace
    annotations = {
      # C'est cette annotation qui fait le lien avec AWS IAM
      "eks.amazonaws.com/role-arn" = module.iam_eks_role.iam_role_arn
    }
  }
}

# On définit le mapping entre AWS Secrets Manager et Kubernetes
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
      secretStoreRef = {
        name = "aws-secretsmanager"
        kind = "ClusterSecretStore"
      }
      target = {
        name = "db-credentials" # Nom du secret créé dans K8s
      }
      data = [
        {
          secretKey = "DATABASE_URL" # La clé que le Pod verra
          remoteRef = {
            key = "${var.project_name}/${var.env}/${var.name}/db-url" # Le nom du secret dans AWS
          }
        }
      ]
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

  role_policy_arns = {
    policy = aws_iam_policy.secrets_policy.arn
  }
}

# La politique qui donne le droit de lire le secret dans Secrets Manager
resource "aws_iam_policy" "secrets_policy" {
  name        = "${var.project_name}-${var.env}-${var.name}-secrets-policy"
  description = "Autorise la lecture du secret de la base de données"

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Action   = "secretsmanager:GetSecretValue"
        Effect   = "Allow"
        Resource = var.db_secret_arn
      }
    ]
  })
}