# infrastructure/modules/kubernetes/argocd/bootstrap/main.tf

resource "kubernetes_manifest" "root_seed" {
  manifest = {
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
        path           = "infrastructure/argocd/bootstrap" # Dossier contenant l'ApplicationSet
        targetRevision = var.target_revision
        helm = {
          # Injecte les variables
          parameters = [
            { name = "global.repoUrl",            value = var.repository_url },
            { name = "global.env",                value = var.env },
            { name = "global.clusterName",         value = var.cluster_name },
            { name = "global.certificateArn",      value = var.ssl_certificate_arn },
            { name = "global.lbControllerRoleArn", value = var.addons_iam_roles["lb_controller"] },
            { name = "global.karpenterRoleArn",   value = var.addons_iam_roles["karpenter"] },
            { name = "global.externalDnsRoleArn",  value = var.addons_iam_roles["external_dns"] },
            { name = "global.certManagerRoleArn",  value = var.addons_iam_roles["cert_manager"] }
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
}