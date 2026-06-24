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
