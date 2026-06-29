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

# --- CMP VALUES (runtime endpoints for the envsubst plugin) ------------------
# The argocd-repo-server envsubst sidecar (modules/.../server) sources this file
# at render time to resolve the ${VAR} endpoints in the Kustomize workload overlay
# (k8s/overlays/staging). These are non-secret in-VPC endpoints; kept as a Secret
# only to match the repo-server mount/RBAC pattern. Empty values are fine for envs
# that don't use the overlay (dev), where the plugin is never invoked.
resource "kubernetes_secret" "cmp_envsubst_values" {
  metadata {
    name      = "cmp-envsubst-values"
    namespace = "argocd"
    labels    = { "argocd.argoproj.io/managed-by" = "helm" }
  }

  data = {
    env = <<-ENV
      MSK_BOOTSTRAP_BROKERS_SASL_SCRAM=${var.msk_bootstrap_brokers}
      ELASTICACHE_CONFIG_ENDPOINT=${var.elasticache_endpoint}
      OPENSEARCH_ENDPOINT=${var.opensearch_endpoint}
      ACM_CERTIFICATE_ARN=${var.ssl_certificate_arn}
      AUTH_JWKS_URL=${var.auth_jwks_url}
      KEYCLOAK_TOKEN_ENDPOINT=${var.keycloak_token_endpoint}
    ENV
  }

  depends_on = [var.server_dependency]
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
      region              = var.region
      env                 = var.env
      repository_url      = var.repository_url
      target_revision     = var.target_revision
      clusterName         = var.cluster_name
      clusterEndpoint     = var.cluster_endpoint
      vpcId               = var.vpc_id
      certificateArn      = var.ssl_certificate_arn
      certManagerRoleArn  = var.addons_iam_roles["cert_manager"]
      karpenterRoleArn    = var.addons_iam_roles["karpenter"]
      lbControllerRoleArn = var.addons_iam_roles["lb_controller"]
      externalDnsRoleArn  = var.addons_iam_roles["external_dns"]
      # Optional: only set where the IRSA role exists (staging/prod via
      # enable_external_secrets). Empty on dev, which doesn't run ESO.
      externalSecretsRoleArn = lookup(var.addons_iam_roles, "external_secrets", "")
    }
  })

  commit_message      = "chore: update global params from terraform [skip ci]"
  overwrite_on_create = true
}
