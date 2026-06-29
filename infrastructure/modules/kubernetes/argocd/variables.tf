# infrastructure/modules/kubernetes/argocd/variables.tf

variable "argocd_version" { type = string }

variable "env" { type = string }
variable "region" { type = string }
variable "repository_url" { type = string }
variable "target_revision" { type = string }

variable "cluster_name" { type = string }
variable "cluster_endpoint" { type = string }
variable "cluster_ca_certificate" { type = string }

variable "vpc_id" { type = string }
variable "addons_iam_roles" { type = map(string) }
variable "ssl_certificate_arn" { type = string }
variable "addons" { type = any }
variable "tags" { type = map(string) }

# --- CMP envsubst values (runtime data-store endpoints) ----------------------
# Resolve the ${VAR} endpoints in the Kustomize workload overlay. Default "" so
# the dev cluster (in-cluster data stores, legacy Helm path) is unaffected.
variable "msk_bootstrap_brokers" {
  type    = string
  default = ""
}
variable "elasticache_endpoint" {
  type    = string
  default = ""
}
variable "opensearch_endpoint" {
  type    = string
  default = ""
}
variable "auth_jwks_url" {
  type    = string
  default = ""
}
variable "keycloak_token_endpoint" {
  type    = string
  default = ""
}

# Per-env bootstrap wiring (defaults preserve the original dev behavior).
variable "bootstrap_path" {
  type    = string
  default = "infrastructure/argocd/bootstrap"
}
variable "global_params_file" {
  type    = string
  default = "global-params.json"
}
