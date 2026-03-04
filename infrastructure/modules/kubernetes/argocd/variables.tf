# infrastructure/modules/kubernetes/argocd/variables.tf

variable "cluster_name" {
  description = "Nom du cluster EKS"
  type        = string
}

variable "repository_url" {
  description = "URL du dépôt Git contenant les manifests ArgoCD (App-of-Apps)"
  type        = string
}

variable "repository_path" {
  description = "Chemin dans le dépôt Git vers le dossier argocd"
  type        = string
  default     = "infrastructure/argocd-root"
}

variable "target_revision" {
  description = "Branche ou révision Git à utiliser"
  type        = string
  default     = "main"
}

variable "addons_iam_roles" {
  description = "Map des ARNs des rôles IAM créés par le module Identity"
  type        = map(string)
  default     = {}
}

variable "cluster_endpoint" {
  description = "L'URL de l'API server Kubernetes"
  type        = string
}

variable "cluster_ca_certificate" {
  description = "Certificat CA du cluster (format base64)"
  type        = string
}