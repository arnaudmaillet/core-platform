# infrastructure/modules/kubernetes/argocd/bootstrap/variables.tf

variable "repository_url"  { type = string }
variable "target_revision" { type = string }