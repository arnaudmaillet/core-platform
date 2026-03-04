# infrastructure/modules/kubernetes/argocd/main.tf

# --- 1. INSTALLATION D'ARGOCD VIA HELM ---
resource "helm_release" "argocd" {
  name             = "argocd"
  repository       = "https://argoproj.github.io/argo-helm"
  chart            = "argo-cd"
  namespace        = "argocd"
  create_namespace = true
  version          = "7.7.0"

  # --- CONFIGURATION DE RÉSILIENCE SRE ---
  # On augmente le timeout et on désactive le 'wait' pour le bootstrap initial.
  # Cela permet à Terraform de rendre la main dès que l'ordre d'installation est validé,
  # laissant Kubernetes stabiliser les pods en arrière-plan.
  timeout          = 600   # 10 minutes
  wait             = false 
  cleanup_on_fail  = true
  replace          = true  

  disable_webhooks = true
  force_update     = true

  # Syntaxe propre pour les paramètres Helm
  set = [{
    name  = "server.service.type"
    value = "ClusterIP"
  }]
}

# --- 2. APPLICATION RACINE (APP-OF-APPS) ---
# Cette ressource crée l'objet qui va piloter tout ton GitOps.
resource "kubernetes_manifest" "root_application" {
  # Dépendance explicite : on attend que Helm ait injecté les CRDs ArgoCD
  depends_on = [helm_release.argocd]

  manifest = {
    apiVersion = "argoproj.io/v1alpha1"
    kind       = "Application"
    metadata = {
      name      = "root-bootstrap"
      namespace = "argocd"
      # --- CONSEIL SRE ---
      # En phase de DEV/TEST, commente le finalizer ci-dessous pour éviter
      # que le destroy ne bloque indéfiniment.
      # finalizers = [
      #   "resources-finalizer.argocd.argoproj.io"
      # ]
    }
    spec = {
      project = "default"
      source = {
        repoURL        = var.repository_url
        path           = var.repository_path
        targetRevision = var.target_revision
        helm = {
          # Injection dynamique des rôles IAM créés par le module Identity
          parameters = [
            { name = "global.karpenterRoleArn", value = var.addons_iam_roles["karpenter"] },
            { name = "global.externalDnsRoleArn", value = var.addons_iam_roles["external_dns"] },
            { name = "global.lbControllerRoleArn", value = var.addons_iam_roles["lb_controller"] },
            { name = "global.certManagerRoleArn", value = var.addons_iam_roles["cert_manager"] },
            { name = "global.clusterName", value = var.cluster_name }
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
      }
    }
  }
}