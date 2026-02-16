# infrastructure/modules/kubernetes/addons/argocd.tf

resource "helm_release" "argocd" {
  name             = "argocd"
  repository       = "https://argoproj.github.io/argo-helm"
  chart            = "argo-cd"
  version          = "7.7.0"
  namespace        = "argocd"
  create_namespace = true

  set = [
    {
      name  = "server.service.type"
      value = "ClusterIP"
    },
    {
      name  = "server.extraArgs"
      # On utilise {--insecure} pour simplifier le dialogue avec l'ALB
      value = "{--insecure}"
    },
    {
      name  = "server.resources.limits.memory"
      value = "512Mi"
    },
    {
      name  = "server.configs.params.server\\.insecure"
      value = "true"
    },
    {
      name  = "global.nodeSelector.intent"
      value = "management"
    }
  ]

  depends_on = [
    helm_release.aws_lb_controller,
    kubernetes_storage_class_v1.gp3
  ]
}

data "aws_elb_hosted_zone_id" "main" {}

resource "aws_route53_record" "argocd" {
  zone_id = var.route53_zone_id
  name    = "argocd.core-platform.click"
  type    = "CNAME"
  ttl     = 300

  records = [
    try(kubernetes_ingress_v1.argocd_server.status[0].load_balancer[0].ingress[0].hostname, "pending.waiting.for.alb")
  ]

  depends_on = [kubernetes_ingress_v1.argocd_server]
}

# Ingress
resource "kubernetes_ingress_v1" "argocd_server" {
  metadata {
    name      = "argocd-server-ingress"
    namespace = "argocd"
    annotations = {
      # On dit à K8s d'utiliser l'ALB AWS
      "kubernetes.io/ingress.class"      = "alb"
      "alb.ingress.kubernetes.io/scheme" = "internet-facing"

      # Configuration SSL avec ton certificat généré dans le module EKS
      "alb.ingress.kubernetes.io/certificate-arn" = var.ssl_certificate_arn
      "alb.ingress.kubernetes.io/listen-ports"    = "[{\"HTTP\": 80}, {\"HTTPS\": 443}]"
      "alb.ingress.kubernetes.io/actions.ssl-redirect" = "{\"Type\": \"redirect\", \"RedirectConfig\": { \"Protocol\": \"HTTPS\", \"Port\": \"443\", \"StatusCode\": \"HTTP_301\"}}"

      # Configuration pour le backend
      "alb.ingress.kubernetes.io/backend-protocol" = "HTTP"
      "alb.ingress.kubernetes.io/target-type"     = "ip"
    }
  }

  spec {
    ingress_class_name = "alb"

    rule {
      host = "argocd.core-platform.click"
      http {
        # 1. On définit d'abord la redirection (elle sera prioritaire)
        path {
          path      = "/"
          path_type = "Prefix"
          backend {
            service {
              name = "ssl-redirect"
              port {
                name = "use-annotation"
              }
            }
          }
        }

        # 2. Puis le service réel
        path {
          path      = "/"
          path_type = "Prefix"
          backend {
            service {
              name = "argocd-server"
              port {
                number = 80
              }
            }
          }
        }
      }
    }
  }

  depends_on = [helm_release.argocd]
}