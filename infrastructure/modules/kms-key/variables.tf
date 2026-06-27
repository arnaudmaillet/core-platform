# infrastructure/modules/kms-key/variables.tf

variable "alias" {
  type        = string
  description = "Alias name (without the 'alias/' prefix), e.g. core-platform-staging-audit-kek."
}

variable "description" {
  type        = string
  description = "Key description."
  default     = "Customer-managed KMS key"
}

variable "deletion_window_in_days" {
  type        = number
  description = "Waiting period before the key is deleted after scheduling."
  default     = 30
}

variable "enable_key_rotation" {
  type        = bool
  description = "Enable annual automatic key rotation."
  default     = true
}

variable "tags" {
  type        = map(string)
  description = "Tags applied to the key."
  default     = {}
}
