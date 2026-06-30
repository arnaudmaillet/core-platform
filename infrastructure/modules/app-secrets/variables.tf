# infrastructure/modules/app-secrets/variables.tf

variable "name" {
  type        = string
  description = "Env-scoped prefix, e.g. core-platform-staging. Secret names are <name>-{media-s3,audit-crypto,auth-secrets}; the ExternalSecrets in k8s/overlays/<env> reference these literal names."
}

variable "media_bucket_arn" {
  type        = string
  description = "ARN of the media object-store bucket. The media static-key IAM user is scoped to read/write here (rusty-s3 SigV4 needs static keys, not IRSA)."
}

variable "audit_worm_bucket_arn" {
  type        = string
  description = "ARN of the audit WORM bucket. The audit object-store + witness static-key IAM users are scoped to write here (append-only; Object-Lock blocks deletes regardless)."
}

variable "audit_kms_key_arn" {
  type        = string
  description = "ARN of the audit KEK (KMS). The audit static-key users need GenerateDataKey/Decrypt to write/read the SSE-KMS WORM objects. NOTE: distinct from the app-level env KEK (kek_base64) this module generates for crypto-shred."
}

variable "secret_recovery_window_days" {
  type        = number
  description = "Secrets Manager recovery window (days). 0 = delete immediately on destroy (disposable/staging); 7-30 = recoverable (prod)."
  default     = 30
  validation {
    condition     = var.secret_recovery_window_days == 0 || (var.secret_recovery_window_days >= 7 && var.secret_recovery_window_days <= 30)
    error_message = "secret_recovery_window_days must be 0 or between 7 and 30."
  }
}

variable "tags" {
  type        = map(string)
  description = "Tags applied to created resources."
  default     = {}
}
