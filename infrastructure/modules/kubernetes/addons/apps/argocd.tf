# infrastructure/modules/kubernetes/addons/argocd.tf

# 1. INSTALLATION DU MOTEUR ARGOCD
resource "helm_release" "argocd" {
  name             = "argocd"
  repository       = "https://argoproj.github.io/argo-helm"
  chart            = "argo-cd"
  version          = "7.7.0"
  namespace        = "argocd"
  create_namespace = true

  wait          = true
  wait_for_jobs = true
  timeout       = 600

  values = [
    yamlencode({
      global = {
        nodeSelector = { intent = "system" }
        tolerations = [{
          key      = "CriticalAddonsOnly"
          operator = "Equal"
          value    = "true"
          effect   = "NoSchedule"
        }]
      }
      server = {
        extraArgs = ["--insecure"]
        service   = { type = "ClusterIP" }
      }
    })
  ]
}

# 2. DELAI DE CONVERGENCE
resource "time_sleep" "wait_for_argocd" {
  depends_on      = [helm_release.argocd]
  create_duration = "30s"
}

# 3. LA ROOT APPLICATION (Le pont GitOps)
# C'est ici qu'on injecte dynamiquement tes ARNs de Terraform vers Git
resource "kubectl_manifest" "root_application" {
  yaml_body = yamlencode({
    apiVersion = "argoproj.io/v1alpha1"
    kind       = "Application"
    metadata = {
      name      = "root-bootstrap"
      namespace = "argocd"
      finalizers = [
        "resources-finalizer.argocd.argoproj.io"
      ]
    }
    spec = {
      project = "default"
      source = {
        repoURL        = "https://github.com/arnaudmaillet/core-platform.git"
        targetRevision = "develop"
        path           = "infrastructure/argocd"
        helm = {
          parameters = [
            { name = "global.env", value = "dev" },
            { name = "global.clusterName", value = var.cluster_name },
            { name = "global.karpenterRoleArn", value = var.karpenter_controller_role_arn },
            { name = "global.certificateArn", value = var.ssl_certificate_arn },
            { name = "global.externalDnsRoleArn", value = var.external_dns_role_arn }
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
  })
  depends_on = [time_sleep.wait_for_argocd]
}
