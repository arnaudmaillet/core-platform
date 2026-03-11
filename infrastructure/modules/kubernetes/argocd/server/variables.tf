# infrastructure/modules/kubernetes/argocd/server/variables.tf

variable "env" { type = string }

variable "cluster_name" {
  description = "Nom du cluster EKS"
  type        = string
}

variable "cluster_endpoint" {
  description = "Endpoint de l'API Kubernetes"
  type        = string
}

variable "cluster_ca_certificate" {
  description = "Certificat CA du cluster (format base64)"
  type        = string
}

variable "argocd_version" {
  description = "Version du Chart Helm ArgoCD"
  type        = string
  default     = "7.7.0"
}

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