# infrastructure/modules/kubernetes/addons/k6-operator.tf

resource "helm_release" "k6_operator" {
  name             = "k6-operator"
  repository       = "https://grafana.github.io/helm-charts"
  chart            = "k6-operator"
  version          = "3.6.0"
  namespace        = "k6-operator-system"
  create_namespace = true

  # On s'assure que le manager de l'opérateur tourne sur nos nodes système
  values = [
    yamlencode({
      manager = {
        nodeSelector = {
          intent = "system"
        }
        tolerations = [{
          key      = "CriticalAddonsOnly"
          operator = "Equal"
          value    = "true"
          effect   = "NoSchedule"
        }]
      }
    })
  ]
}