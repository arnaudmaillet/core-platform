# infrastructure/modules/kubernetes/argocd/bootstrap/variables.tf

variable "repository_url"   { type = string }
variable "target_revision"  { type = string }
variable "cluster_name"     { type = string }
variable "ssl_certificate_arn" { type = string }
variable "addons_iam_roles" { type = map(string) }
variable "cluster_endpoint"       { type = string }

variable "server_dependency" { 
  type    = any 
  default = null
  description = "Permet de forcer l'attente de l'installation du serveur ArgoCD"
}