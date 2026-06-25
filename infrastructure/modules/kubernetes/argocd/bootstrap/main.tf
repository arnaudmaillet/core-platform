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
  yaml_body  = <<YAML
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
    path: ${var.bootstrap_path}
    directory:
      exclude: '${var.global_params_file}'
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
  file       = "infrastructure/argocd/bootstrap/${var.global_params_file}"

  content = jsonencode({
    global = {
      region                  = var.region
      env                     = var.env
      repository_url          = var.repository_url
      target_revision         = var.target_revision
      clusterName             = var.cluster_name
      clusterEndpoint         = var.cluster_endpoint
      vpcId                   = var.vpc_id
      certificateArn          = var.ssl_certificate_arn
      certManagerRoleArn      = var.addons_iam_roles["cert_manager"]
      karpenterRoleArn        = var.addons_iam_roles["karpenter"]
      lbControllerRoleArn     = var.addons_iam_roles["lb_controller"]
      externalDnsRoleArn      = var.addons_iam_roles["external_dns"]
      # Optional: only set where the IRSA role exists (staging/prod via
      # enable_external_secrets). Empty on dev, which doesn't run ESO.
      externalSecretsRoleArn  = lookup(var.addons_iam_roles, "external_secrets", "")
    }
  })

  commit_message      = "chore: update global params from terraform [skip ci]"
  overwrite_on_create = true
}
