# infrastructure/modules/kubernetes/argocd/server/main.tf

resource "helm_release" "argocd" {
  name             = "argocd"
  repository       = "https://argoproj.github.io/argo-helm"
  chart            = "argo-cd"
  namespace        = "argocd"
  create_namespace = true
  version          = var.argocd_version
  cleanup_on_fail  = true
  wait             = true
  timeout          = 300

  values = [
    yamlencode({
      commonLabels = {
        "argocd.argoproj.io/managed-by" = "helm"
      }
      server = {
        extraArgs = ["--insecure"]
        config = {
          "server.insecure" = "true"
        }
        service = { type = "ClusterIP" }
      }
      redis = { enabled = true }
    })
  ]
}
