# infrastructure/modules/kubernetes/argocd/server/variables.tf

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