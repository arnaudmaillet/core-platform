# infrastructure/modules/kubernetes/external-secret/variables.tf

variable "project_name" {
  description = "Nom du projet pour le nommage des ressources IAM"
  type        = string
}

variable "env" {
  description = "Environnement (dev, staging, prod)"
  type        = string
}

variable "region" {
  description = "Région AWS"
  type        = string
  default     = "us-east-1"
}

variable "oidc_provider_arn" {
  description = "ARN du provider OIDC de l'EKS (pour IRSA)"
  type        = string
}