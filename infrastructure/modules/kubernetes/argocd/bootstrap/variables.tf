# infrastructure/modules/kubernetes/argocd/bootstrap/variables.tf

variable "env" { type = string }
variable "cluster_name"           { type = string }
variable "cluster_endpoint"       { type = string }
variable "cluster_ca_certificate" { type = string }

variable "repository_url" {
  description = "URL SSH ou HTTPS du repo GitOps"
  type        = string
}

variable "repository_path" {
  description = "Chemin vers le dossier racine d'ArgoCD (ex: infrastructure/argocd-root)"
  type        = string
}

variable "target_revision" {
  description = "Branche ou Tag à suivre (ex: main)"
  type        = string
  default     = "main"
}

variable "addons_iam_roles" {
  description = "Map des ARNs des rôles IAM (output du module identity)"
  type        = map(string)
}

variable "ssl_certificate_arn" {
  description = "ARN du certificat ACM pour les Ingress (ALB)"
  type        = string
}