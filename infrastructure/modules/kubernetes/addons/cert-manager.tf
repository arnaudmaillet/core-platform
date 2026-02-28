# infrastructure/modules/kubernetes/addons/cert-manager.tf

resource "helm_release" "cert_manager" {
  name             = "cert-manager"
  repository       = "https://charts.jetstack.io"
  chart            = "cert-manager"
  version          = "v1.13.3"
  namespace        = "cert-manager"
  create_namespace = true

  set = {
    name  = "installCRDs"
    value = "true"
  }

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