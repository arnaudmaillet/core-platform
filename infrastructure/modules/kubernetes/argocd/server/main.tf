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
      # --- CONFIGURATION SERVEUR ---
      server = {
        extraArgs = ["--insecure"]
        config = {
          "server.insecure" = "true"
        }
        service = {
          type = "ClusterIP"
        }
      }

      # --- REDIS (CACHE) ---
      redis = {
        enabled = true
      }

      # --- INJECTION DE L'APPLICATION RACINE (BOOTSTRAP) ---
      extraObjects = [
        {
          apiVersion = "argoproj.io/v1alpha1"
          kind       = "Application"
          metadata = {
            name      = "root-bootstrap"
            namespace = "argocd"
          }
          spec = {
            project = "default"
            source = {
              repoURL        = var.repository_url
              targetRevision = var.target_revision
              path           = "infrastructure/argocd/bootstrap"
            }
            destination = {
              server    = "https://kubernetes.default.svc"
              namespace = "argocd"
            }
            syncPolicy = {
              automated = {
                prune    = true
                selfHeal = true
              }
              syncOptions = ["CreateNamespace=true"]
            }
          }
        }
      ]
    })
  ]
}