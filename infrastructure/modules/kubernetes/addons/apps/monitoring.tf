# infrastructure/modules/kubernetes/addons/monitoring.tf

resource "helm_release" "prometheus_stack" {
  name             = "monitoring"
  repository       = "https://prometheus-community.github.io/helm-charts"
  chart            = "kube-prometheus-stack"
  version          = "56.0.0"
  namespace        = "monitoring"
  create_namespace = true

  values = [
    yamlencode({
      # 1. Grafana Configuration
      grafana = {
        nodeSelector = { intent = "system" }
        tolerations = [
          {
            key      = "CriticalAddonsOnly"
            operator = "Equal"
            value    = "true"
            effect   = "NoSchedule"
          }
        ]
        ingress = {
          enabled          = true
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

      # 2. Prometheus Operator Configuration
      prometheusOperator = {
        nodeSelector = { intent = "system" }
        tolerations = [
          {
            key      = "CriticalAddonsOnly"
            operator = "Equal"
            value    = "true"
            effect   = "NoSchedule"
          }
        ]
      }

      # 3. Prometheus Instance Configuration
      prometheus = {
        enabled = true
        prometheusSpec = {
          nodeSelector = { intent = "system" }
          tolerations = [
            {
              key      = "CriticalAddonsOnly"
              operator = "Equal"
              value    = "true"
              effect   = "NoSchedule"
            }
          ]
          
          # LA CORRECTION EST ICI :
          # On remplace le bloc "v1" par "volumeClaimTemplate"
          storageSpec = {
            volumeClaimTemplate = {
              spec = {
                storageClassName = "gp3" # Doit correspondre à la StorageClass créée dans le module CORE
                accessModes      = ["ReadWriteOnce"]
                resources = {
                  requests = {
                    storage = "20Gi" # Ta cible de 20 Go
                  }
                }
              }
            }
          }
        }
      }

      # 4. Alertmanager Configuration
      alertmanager = {
        alertmanagerSpec = {
          nodeSelector = { intent = "system" }
          tolerations = [
            {
              key      = "CriticalAddonsOnly"
              operator = "Equal"
              value    = "true"
              effect   = "NoSchedule"
            }
          ]
        }
      }

      # 5. Kube-State-Metrics & Node Exporter
      kube-state-metrics = {
        nodeSelector = { intent = "system" }
        tolerations = [
          {
            key      = "CriticalAddonsOnly"
            operator = "Equal"
            value    = "true"
            effect   = "NoSchedule"
          }
        ]
      }
      
      nodeExporter = {
        # Le node exporter doit monitorer tous les nodes (system, db, apps)
        tolerations = [
          {
            operator = "Exists"
          }
        ]
      }
    })
  ]
}