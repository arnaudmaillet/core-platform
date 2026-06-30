# infrastructure/modules/s3-bucket/variables.tf

variable "name" {
  type        = string
  description = "Globally-unique bucket name."
}

variable "versioning_enabled" {
  type        = bool
  description = "Enable versioning (forced on when object_lock_mode is set)."
  default     = true
}

variable "object_lock_mode" {
  type        = string
  description = "'' (off), 'GOVERNANCE', or 'COMPLIANCE'. COMPLIANCE = true WORM."
  default     = ""
  validation {
    condition     = contains(["", "GOVERNANCE", "COMPLIANCE"], var.object_lock_mode)
    error_message = "object_lock_mode must be one of '', 'GOVERNANCE', 'COMPLIANCE'."
  }
}

variable "object_lock_retention_days" {
  type        = number
  description = "Default WORM retention in days (only used when object_lock_mode is set)."
  default     = 2555 # ~7y, a common compliance evidence floor
}

variable "kms_key_arn" {
  type        = string
  description = "SSE-KMS key ARN. Empty string => SSE-S3 (AES256)."
  default     = ""
}

variable "cors_enabled" {
  type        = bool
  description = "Add a CORS rule for browser presigned PUT/GET (media)."
  default     = false
}

variable "cors_allowed_origins" {
  type        = list(string)
  description = "Origins allowed by the CORS rule."
  default     = ["*"]
}

variable "force_destroy" {
  type        = bool
  description = "Allow `terraform destroy` to delete a non-empty bucket (deletes all objects + versions first). Safe default off; enable only for ephemeral/re-derivable buckets (NOT WORM/compliance)."
  default     = false
}

variable "tags" {
  type        = map(string)
  description = "Tags applied to the bucket."
  default     = {}
}
