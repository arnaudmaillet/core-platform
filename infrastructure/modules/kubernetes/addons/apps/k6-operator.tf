# infrastructure/modules/kubernetes/addons/k6-operator.tf

resource "kubernetes_namespace_v1" "k6_system" {
  metadata {
    name = "k6-operator-system"

    # --- LE FIX : On donne à Helm les clés du namespace ---
    labels = {
      "app.kubernetes.io/managed-by" = "Helm"
    }
    annotations = {
      "meta.helm.sh/release-name"      = "k6-operator"
      "meta.helm.sh/release-namespace" = "k6-operator-system"
    }
    # ------------------------------------------------------
  }
}

resource "helm_release" "k6_operator" {
  name       = "k6-operator"
  repository = "https://grafana.github.io/helm-charts"
  chart      = "k6-operator"
  version    = "3.6.0"
  
  namespace  = kubernetes_namespace_v1.k6_system.metadata[0].name
  
  # On laisse Terraform gérer le namespace via la ressource ci-dessus
  create_namespace = false 

  cleanup_on_fail = true
  force_update    = true
  wait            = true

  values = [
    yamlencode({
      manager = {
        nodeSelector = { intent = "system" }
        tolerations = [{
          key      = "CriticalAddonsOnly"
          operator = "Equal"
          value    = "true"
          effect   = "NoSchedule"
        }]
      }
    })
  ]

  lifecycle {
    prevent_destroy = false
  }

  depends_on = [kubernetes_namespace_v1.k6_system]
}