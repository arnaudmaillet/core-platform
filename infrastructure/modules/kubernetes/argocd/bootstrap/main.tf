# infrastructure/modules/kubernetes/argocd/bootstrap/main.tf

terraform {
  required_providers {
    github = {
      source  = "integrations/github"
      version = "~> 6.0"
    }
    kubernetes = {
      source = "hashicorp/kubernetes"
    }
    kubectl = {
      source  = "gavinbunney/kubectl"
      version = ">= 1.14.0"
    }
    local = {
      source = "hashicorp/local"
    }
    null = {
      source = "hashicorp/null"
    }
  }
}

resource "kubectl_manifest" "root_app" {
  yaml_body = <<YAML
apiVersion: argoproj.io/v1alpha1
kind: Application
metadata:
  name: root-bootstrap
  namespace: argocd
spec:
  project: default
  source:
    repoURL: ${var.repository_url}
    targetRevision: ${var.target_revision}
    path: infrastructure/argocd/bootstrap
    directory:
      exclude: 'global-params.json'
  destination:
    server: https://kubernetes.default.svc
    namespace: argocd
  syncPolicy:
    automated:
      prune: true
      selfHeal: true
    syncOptions:
      - CreateNamespace=true
      - ServerSideApply=true
YAML
  depends_on = [var.server_dependency]
}

# --- DYNAMIC PARAMETERS (GIT SOURCE OF TRUTH) ---
resource "github_repository_file" "argocd_params" {
  repository = "core-platform"
  branch     = var.target_revision
  file       = "infrastructure/argocd/bootstrap/global-params.json"
  
  content = jsonencode({
    global = {
      clusterName         = var.cluster_name
      certificateArn      = var.ssl_certificate_arn
      clusterEndpoint     = var.cluster_endpoint
      karpenterRoleArn    = var.addons_iam_roles["karpenter"]
      lbControllerRoleArn = var.addons_iam_roles["lb_controller"]
      externalDnsRoleArn  = var.addons_iam_roles["external_dns"]
      target_revision     = var.target_revision
      repository_url      = var.repository_url
      env                 = var.env
    }
  })

  commit_message      = "chore: update global params from terraform [skip ci]"
  overwrite_on_create = true
}