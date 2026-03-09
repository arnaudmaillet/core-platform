# infrastructure/modules/kubernetes/argocd/bootstrap/main.tf

resource "kubernetes_manifest" "root_application" {
  manifest = {
    apiVersion = "argoproj.io/v1alpha1"
    kind       = "Application"
    metadata = {
      name      = "root-bootstrap"
      namespace = "argocd"
      # On ne met pas de finalizer ici en environnement de DEV 
      # pour permettre un 'terragrunt destroy' fluide.
    }
    spec = {
      project = "default"
      source = {
        repoURL        = var.repository_url
        path           = var.repository_path
        targetRevision = var.target_revision
        helm = {
          parameters = [
            { name = "global.env",                value = var.env },
            { name = "global.karpenterRoleArn",   value = var.addons_iam_roles["karpenter"] },
            { name = "global.lbControllerRoleArn", value = var.addons_iam_roles["lb_controller"] },
            { name = "global.externalDnsRoleArn",  value = var.addons_iam_roles["external_dns"] },
            { name = "global.certManagerRoleArn",  value = var.addons_iam_roles["cert_manager"] },
            { name = "global.certificateArn",      value = var.ssl_certificate_arn },
            { name = "global.clusterName",         value = var.cluster_name }
          ]
        }
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
}