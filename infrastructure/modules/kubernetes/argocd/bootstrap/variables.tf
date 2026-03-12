# infrastructure/modules/kubernetes/argocd/bootstrap/variables.tf

variable "repository_url"   { type = string }
variable "target_revision"  { type = string }
variable "cluster_name"     { type = string }
variable "ssl_certificate_arn" { type = string }
variable "addons_iam_roles" { type = map(string) }