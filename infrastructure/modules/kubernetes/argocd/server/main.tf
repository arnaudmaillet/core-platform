# infrastructure/modules/kubernetes/argocd/server/main.tf

resource "helm_release" "argocd" {
  name             = "argocd"
  repository       = "https://argoproj.github.io/argo-helm"
  chart            = "argo-cd"
  namespace        = "argocd"
  create_namespace = true
  version          = var.argocd_version

  values = [
    yamlencode({
      # Méthode SRE la plus robuste : Injection directe d'objets Kubernetes via Helm
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
              helm = {
                parameters = [
                  { name = "global.repoUrl", value = var.repository_url },
                  { name = "global.env", value = var.env },
                  { name = "global.clusterName", value = var.cluster_name },
                  { name = "global.certificateArn", value = var.ssl_certificate_arn },
                  { name = "global.lbControllerRoleArn", value = var.addons_iam_roles["lb_controller"] },
                  { name = "global.karpenterRoleArn", value = var.addons_iam_roles["karpenter"] },
                  { name = "global.externalDnsRoleArn", value = var.addons_iam_roles["external_dns"] },
                  { name = "global.certManagerRoleArn", value = var.addons_iam_roles["cert_manager"] }
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
        }
      ]

      server = {
        extraArgs = ["--insecure"]
        config    = { "server.insecure" = "true" }
        service   = { type = "ClusterIP" }
      }
      redis = { enabled = true }
    })
  ]
}
