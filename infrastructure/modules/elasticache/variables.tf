# infrastructure/modules/elasticache/variables.tf

variable "name" {
  description = "ElastiCache replication group name (e.g. core-platform-staging)."
  type        = string
}

variable "vpc_id" {
  description = "VPC the cache lives in."
  type        = string
}

variable "subnet_ids" {
  description = "Private subnets for the cache subnet group."
  type        = list(string)
}

variable "allowed_cidr_blocks" {
  description = "CIDRs allowed to reach Redis on 6379 (typically the VPC CIDR)."
  type        = list(string)
}

variable "engine_version" {
  description = "Redis engine version."
  type        = string
  default     = "7.1"
}

variable "node_type" {
  description = "Cache node type."
  type        = string
  default     = "cache.t4g.small"
}

variable "num_node_groups" {
  description = "Number of shards (cluster mode)."
  type        = number
  default     = 2
}

variable "replicas_per_node_group" {
  description = "Read replicas per shard."
  type        = number
  default     = 1
}

variable "tags" {
  description = "Tags applied to all resources."
  type        = map(string)
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
