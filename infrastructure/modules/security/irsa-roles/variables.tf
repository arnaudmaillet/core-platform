# infrastructure/modules/kubernetes/identity/variables.tf

variable "cluster_name" {
  description = "Nom du cluster EKS"
  type        = string
}

variable "oidc_provider_arn" {
  description = "ARN du provider OIDC de EKS"
  type        = string
}

variable "oidc_provider_url" {
  description = "URL du provider OIDC de EKS (sans https://)"
  type        = string
}

variable "tags" {
  description = "Tags à appliquer aux ressources"
  type        = map(string)
  default     = {}
}

variable "node_iam_role_arns" {
  description = "Liste des ARNs des rôles IAM des nodes (nécessaire pour Karpenter PassRole)"
  type        = list(string)
  default     = []
}

variable "iam_policy_json_content" {
  description = "Document JSON définissant les permissions pour l'AWS Load Balancer Controller"
  type        = string
}
variable "enable_external_secrets" {
  description = "Create an IRSA role for External Secrets Operator (staging/prod; reads managed-backend creds from Secrets Manager)."
  type        = bool
  default     = false
}
