# infrastructure/modules/services/microservices/variables.tf

variable "name" {
  description = "Nom du microservice"
  type        = string
}

variable "namespace" {
  description = "Namespace Kubernetes"
  type        = string
  default     = "default"
}

variable "project_name" {
  description = "Nom du projet pour le nommage des ressources"
  type        = string
}

variable "env" {
  description = "Environnement (dev, staging, prod)"
  type        = string
}

variable "image" {
  description = "URL de l'image ECR"
  type        = string
}

# --- Sécurité & Secrets ---

variable "oidc_provider_arn" {
  description = "ARN du provider OIDC de l'EKS (pour IRSA)"
  type        = string
}

variable "db_secret_arn" {
  description = "L'ARN du secret AWS Secrets Manager à autoriser"
  type        = string
}

# --- Configuration Runtime ---

variable "replicas" {
  description = "Nombre d'instances"
  type        = number
  default     = 1
}

variable "port" {
  description = "Port d'écoute du container"
  type        = number
  default     = 50051
}

variable "env_vars" {
  description = "Map des variables d'environnement (hors secrets)"
  type        = map(string)
  default     = {}
}

# --- Ressources (Hyperscale ready) ---

variable "cpu_request"    { default = "100m" }
variable "cpu_limit"      { default = "500m" }
variable "memory_request" { default = "128Mi" }
variable "memory_limit"   { default = "512Mi" }