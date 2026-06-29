# infrastructure/modules/kubernetes/argocd/bootstrap/variables.tf

variable "region" { type = string }
variable "env" { type = string }
variable "repository_url" { type = string }
variable "target_revision" { type = string }
variable "vpc_id" { type = string }
variable "cluster_name" { type = string }
variable "cluster_endpoint" { type = string }
variable "ssl_certificate_arn" { type = string }
variable "addons_iam_roles" { type = map(string) }

# --- CMP envsubst values (runtime data-store endpoints) ----------------------
# Threaded into the cmp-envsubst-values Secret the repo-server plugin sources.
# Default "" so single-env/dev (no managed data stores, no overlay) is unaffected.
variable "msk_bootstrap_brokers" {
  type        = string
  default     = ""
  description = "MSK SASL/SCRAM bootstrap brokers (CMP var MSK_BOOTSTRAP_BROKERS_SASL_SCRAM)."
}
variable "elasticache_endpoint" {
  type        = string
  default     = ""
  description = "ElastiCache cluster-mode configuration endpoint (CMP var ELASTICACHE_CONFIG_ENDPOINT)."
}
variable "opensearch_endpoint" {
  type        = string
  default     = ""
  description = "OpenSearch VPC endpoint, no scheme (CMP var OPENSEARCH_ENDPOINT)."
}
variable "auth_jwks_url" {
  type        = string
  default     = ""
  description = "Auth JWKS URL (CMP var AUTH_JWKS_URL; empty until Keycloak/auth is provisioned)."
}
variable "keycloak_token_endpoint" {
  type        = string
  default     = ""
  description = "Keycloak token endpoint (CMP var KEYCLOAK_TOKEN_ENDPOINT; empty until provisioned)."
}

# Per-env bootstrap wiring (defaults preserve the original single-env/dev behavior).
variable "bootstrap_path" {
  type        = string
  default     = "infrastructure/argocd/bootstrap"
  description = "Repo path the per-cluster root-bootstrap Application syncs (e.g. .../bootstrap/staging for staging)."
}

variable "global_params_file" {
  type        = string
  default     = "global-params.json"
  description = "Per-env Helm params file (relative to infrastructure/argocd/bootstrap/) written by Terraform and consumed by the appsets."
}

variable "server_dependency" {
  type        = any
  default     = null
  description = "Permet de forcer l'attente de l'installation du serveur ArgoCD"
}
