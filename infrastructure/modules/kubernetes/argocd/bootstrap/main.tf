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
    local = {
      source = "hashicorp/local"
    }
    null = {
      source = "hashicorp/null"
    }
  }
}

# --- ROOT APPLICATION ---
resource "local_file" "root_app_yaml" {
  content  = <<EOF
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
      prune: false
      selfHeal: true
    syncOptions:
      - CreateNamespace=true
      - ServerSideApply=true
EOF
  filename = "${path.module}/root-app.yaml"
}

resource "null_resource" "apply_root_app" {
  depends_on = [local_file.root_app_yaml]

  provisioner "local-exec" {
    command = "sleep 10 && kubectl apply -f ${local_file.root_app_yaml.filename} --validate=false"
  }
}

# --- DYNAMIC PARAMETERS (GIT SOURCE OF TRUTH) ---
resource "github_repository_file" "argocd_params" {
  repository = "core-platform"
  branch     = var.target_revision
  file       = "infrastructure/argocd/bootstrap/global-params.json"
  content = jsonencode({
    clusterName         = var.cluster_name
    certificateArn      = var.ssl_certificate_arn
    clusterEndpoint     = var.cluster_endpoint
    karpenterRoleArn    = var.addons_iam_roles["karpenter"]
    lbControllerRoleArn = var.addons_iam_roles["lb_controller"]
    externalDnsRoleArn  = var.addons_iam_roles["external_dns"]
  })
  commit_message      = "chore: update global params from terraform [skip ci]"
  overwrite_on_create = true
}
