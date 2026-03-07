# infrastructure/modules/kubernetes/argocd/variables.tf

variable "cluster_name"           { type = string }
variable "cluster_endpoint"       { type = string }
variable "cluster_ca_certificate" { type = string }

# --- ArgoCD Server ---
variable "argocd_version" {
  type    = string
  default = "7.7.0"
}

# --- Bootstrap ---
variable "repository_url"  { type = string }
variable "repository_path" { type = string }
variable "target_revision" {
  type    = string
  default = "HEAD"
}

variable "addons_iam_roles" {
  type = map(string)
}

variable "ssl_certificate_arn" {
  type = string
}

# --- Addons EKS ---
variable "addons" {
  type = map(object({
    addon_version            = optional(string)
    service_account_role_arn = optional(string)
  }))
  default = {}
}

variable "tags" {
  type    = map(string)
  default = {}
}