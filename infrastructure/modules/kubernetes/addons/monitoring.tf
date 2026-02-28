# infrastructure/modules/kubernetes/addons/monitoring.tf

resource "helm_release" "prometheus_stack" {
  name             = "kube-prometheus-stack"
  repository       = "https://prometheus-community.github.io/helm-charts"
  chart            = "kube-prometheus-stack"
  version          = "56.0.0" # Utilise une version fixe
  namespace        = "monitoring"
  create_namespace = true

  values = [
    yamlencode({
      grafana = {
        ingress = {
          enabled = true
          ingressClassName = "alb"
          annotations = {
            "external-dns.alpha.kubernetes.io/hostname" = "grafana.core-platform.click"
            "alb.ingress.kubernetes.io/scheme"          = "internet-facing"
            "alb.ingress.kubernetes.io/target-type"     = "ip"
            "alb.ingress.kubernetes.io/certificate-arn" = var.ssl_certificate_arn
            "alb.ingress.kubernetes.io/listen-ports"    = "[{\"HTTP\": 80}, {\"HTTPS\": 443}]"
          }
          hosts = ["grafana.core-platform.click"]
        }
      }
      # Optionnel : On place les composants sur les nodes system
      prometheus = {
        prometheusSpec = {
          nodeSelector = { intent = "system" }
        }
      }
    })
  ]
}