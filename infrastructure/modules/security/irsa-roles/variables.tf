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

# ── Application IRSA (staging/prod) ────────────────────────────────────────────
# These gate per-app roles for the two services that need direct AWS access.
# Each role is created ONLY when its resource ARN is set (empty => count 0), so
# dev (no managed buckets/KMS) is unaffected. The *_service_accounts lists are the
# IRSA trust subjects in "<namespace>:<sa-name>" form (env-prefixed at the overlay,
# e.g. "default:staging-audit-server").

variable "audit_kek_arn" {
  description = "ARN of the audit KEK (KMS). Empty disables the audit app IRSA role."
  type        = string
  default     = ""
}

variable "audit_worm_bucket_arn" {
  description = "ARN of the audit WORM (Object-Lock) bucket. Required when audit_kek_arn is set."
  type        = string
  default     = ""
}

variable "audit_service_accounts" {
  description = "Trust subjects (<ns>:<sa>) allowed to assume the audit app role (audit-server + audit-worker)."
  type        = list(string)
  default     = []
}

variable "media_bucket_arn" {
  description = "ARN of the media asset bucket. Empty disables the media app IRSA role."
  type        = string
  default     = ""
}

variable "media_service_accounts" {
  description = "Trust subjects (<ns>:<sa>) allowed to assume the media app role (media-server)."
  type        = list(string)
  default     = []
}
