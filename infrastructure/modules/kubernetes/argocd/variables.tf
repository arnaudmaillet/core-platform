# infrastructure/modules/kubernetes/argocd/variables.tf

variable "cluster_name"           { type = string }
variable "cluster_endpoint"       { type = string }
variable "cluster_ca_certificate" { type = string }
variable "argocd_version"         { type = string }

variable "repository_url"         { type = string }
variable "target_revision"        { type = string }
variable "env"                     { type = string }
variable "addons_iam_roles"        { type = map(string) }
variable "ssl_certificate_arn"     { type = string }
variable "addons"                  { type = any }
variable "tags"                    { type = map(string) }