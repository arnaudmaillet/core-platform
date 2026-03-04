resource "helm_release" "argocd" {
  name             = "argocd"
  repository       = "https://argoproj.github.io/argo-helm"
  chart            = "argo-cd"
  namespace        = "argocd"
  create_namespace = true
  version          = var.argocd_version

  wait             = true 
  timeout          = 600
  # Configuration minimale pour ClusterIP (géré ensuite par Ingress via GitOps)
    values = [
    yamlencode({
      server = {
        service = {
          type = "ClusterIP"
        }
      }
      redis = {
        enabled = true
      }
    })
  ]

  # Optionnel : désactiver l'admin password initial si tu gères le SSO plus tard
  # Mais pour l'instant, on le laisse par défaut.
}