# infrastructure/modules/kubernetes/argocd/server/variables.tf

variable "cluster_name"           { type = string }
variable "cluster_endpoint"       { type = string }
variable "cluster_ca_certificate" { type = string }
variable "argocd_version"         { type = string }