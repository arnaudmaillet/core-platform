# infrastructure/modules/opensearch/variables.tf

variable "name" {
  type        = string
  description = "Domain name / resource prefix (e.g. core-platform-staging)."
}

variable "vpc_id" {
  type        = string
  description = "VPC the domain's client security group lives in."
}

variable "subnet_ids" {
  type        = list(string)
  description = "Private subnets for the domain ENIs (one per AZ for zone awareness)."
}

variable "allowed_cidr_blocks" {
  type        = list(string)
  description = "CIDRs allowed to reach the domain on 443 (typically the VPC CIDR)."
}

variable "engine_version" {
  type        = string
  description = "OpenSearch engine version."
  default     = "OpenSearch_2.11"
}

variable "instance_type" {
  type        = string
  description = "Data node instance type."
  default     = "t3.small.search"
}

variable "instance_count" {
  type        = number
  description = "Data node count. >1 enables zone awareness."
  default     = 2
}

variable "volume_size" {
  type        = number
  description = "Per-node EBS (gp3) size in GiB."
  default     = 20
}

variable "master_user" {
  type        = string
  description = "Internal master username for fine-grained access control."
  default     = "search-admin"
}

variable "tags" {
  type        = map(string)
  description = "Tags applied to all resources."
  default     = {}
}

variable "secret_recovery_window_days" {
  type        = number
  description = "Secrets Manager recovery window (days) for this module's secret. 0 = delete immediately on destroy (disposable/staging); 7-30 = recoverable (prod). Default keeps the AWS-standard window."
  default     = 30
  validation {
    condition     = var.secret_recovery_window_days == 0 || (var.secret_recovery_window_days >= 7 && var.secret_recovery_window_days <= 30)
    error_message = "secret_recovery_window_days must be 0 or between 7 and 30."
  }
}
