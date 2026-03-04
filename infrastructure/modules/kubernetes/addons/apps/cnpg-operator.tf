# infrastructure/modules/kubernetes/addons/cnpg-operator.tf

resource "helm_release" "cnpg_operator" {
  name             = "cnpg-operator"
  repository       = "https://cloudnative-pg.github.io/charts"
  chart            = "cloudnative-pg"
  version          = "0.20.0"
  namespace        = "cnpg-system"
  create_namespace = true

  # Idéalement, on le place sur les nodes system pour qu'il soit toujours dispo
  values = [
    yamlencode({
      nodeSelector = {
        intent = "system"
      }
      tolerations = [{
        key      = "CriticalAddonsOnly"
        operator = "Equal"
        value    = "true"
        effect   = "NoSchedule"
      }]
    })
  ]
}