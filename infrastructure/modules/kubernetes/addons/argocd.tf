# infrastructure/modules/kubernetes/addons/argocd.tf

# 1. INSTALLATION DU MOTEUR ARGOCD
resource "helm_release" "argocd" {
  name             = "argocd"
  repository       = "https://argoproj.github.io/argo-helm"
  chart            = "argo-cd"
  version          = "7.7.0"
  namespace        = "argocd"
  create_namespace = true

  wait          = true
  wait_for_jobs = true
  timeout       = 600

  values = [
    yamlencode({
      global = {
        nodeSelector = { intent = "system" }
        tolerations = [{
          key      = "CriticalAddonsOnly"
          operator = "Equal"
          value    = "true"
          effect   = "NoSchedule"
        }]
      }
      server = {
        extraArgs = ["--insecure"]
        service   = { type = "ClusterIP" }
      }
    })
  ]

  depends_on = [helm_release.aws_lb_controller]
}

# 2. DELAI DE CONVERGENCE
resource "time_sleep" "wait_for_argocd" {
  depends_on = [helm_release.argocd]
  create_duration = "30s"
}

# 3. LA ROOT APPLICATION (Le pont GitOps)
resource "kubectl_manifest" "root_application" {
  yaml_body = yamlencode({
    apiVersion = "argoproj.io/v1alpha1"
    kind       = "Application"
    metadata = {
      name      = "root-application"
      namespace = "argocd"
    }
    spec = {
      project = "default"
      source = {
        repoURL        = "https://github.com/arnaudmaillet/core-platform.git"
        targetRevision = "HEAD"
        path           = "infrastructure/argocd" # Dossier où tu mettras tes YAMLs
        helm = {
          parameters = [
            { name = "global.env", value = "dev" },
            { name = "global.clusterName", value = var.cluster_name },
            { name = "global.karpenterRoleArn", value = var.karpenter_controller_role_arn },
            { name = "global.certificateArn", value = var.ssl_certificate_arn },
            { name = "global.externalDnsRoleArn", value = var.external_dns_role_arn }
          ]
        }
      }
      destination = {
        server    = "https://kubernetes.default.svc"
        namespace = "argocd"
      }
      syncPolicy = {
        automated = { prune = true, selfHeal = true }
      }
    }
  })
  depends_on = [time_sleep.wait_for_argocd]
}

# 4. DNS & INGRESS (Pour accéder à l'interface)
resource "kubernetes_ingress_v1" "argocd_server" {
  metadata {
    name      = "argocd-server-ingress"
    namespace = "argocd"
    annotations = {
      "kubernetes.io/ingress.class"               = "alb"
      "alb.ingress.kubernetes.io/scheme"          = "internet-facing"
      "alb.ingress.kubernetes.io/target-type"     = "ip"
      "alb.ingress.kubernetes.io/certificate-arn" = var.ssl_certificate_arn
      "alb.ingress.kubernetes.io/listen-ports"    = "[{\"HTTP\": 80}, {\"HTTPS\": 443}]"
      "alb.ingress.kubernetes.io/actions.ssl-redirect" = "{\"Type\": \"redirect\", \"RedirectConfig\": { \"Protocol\": \"HTTPS\", \"Port\": \"443\", \"StatusCode\": \"HTTP_301\"}}"
    }
  }
  spec {
    ingress_class_name = "alb"
    rule {
      host = "argocd.core-platform.click"
      http {
        path {
          path = "/"
          path_type = "Prefix"
          backend {
            service {
              name = "argocd-server"
              port { number = 80 }
            }
          }
        }
      }
    }
  }
  depends_on = [helm_release.argocd]
}

resource "aws_route53_record" "argocd" {
  zone_id = var.route53_zone_id
  name    = "argocd.core-platform.click"
  type    = "CNAME"
  ttl     = 300
  records = [try(kubernetes_ingress_v1.argocd_server.status[0].load_balancer[0].ingress[0].hostname, "pending")]
}