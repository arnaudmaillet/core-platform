# infrastructure/modules/msk/variables.tf

variable "name" {
  description = "MSK cluster name (e.g. core-platform-staging)."
  type        = string
}

variable "vpc_id" {
  description = "VPC the brokers live in."
  type        = string
}

variable "subnet_ids" {
  description = "Private subnets for the broker ENIs (one per AZ; count must divide number_of_broker_nodes)."
  type        = list(string)
}

variable "allowed_cidr_blocks" {
  description = "CIDRs allowed to reach the brokers (typically the VPC CIDR so in-cluster pods can connect)."
  type        = list(string)
}

variable "kafka_version" {
  description = "Apache Kafka version."
  type        = string
  default     = "3.6.0"
}

variable "number_of_broker_nodes" {
  description = "Total broker count (multiple of the AZ/subnet count)."
  type        = number
  default     = 3
}

variable "broker_instance_type" {
  description = "Broker instance type."
  type        = string
  default     = "kafka.t3.small"
}

variable "broker_ebs_volume_size" {
  description = "Per-broker EBS volume size (GiB)."
  type        = number
  default     = 50
}

variable "default_replication_factor" {
  description = "Broker-side default.replication.factor. Defaults match a 3-broker/3-AZ prod posture; a 2-broker env MUST lower it to 2 (RF cannot exceed broker count)."
  type        = number
  default     = 3
}

variable "min_insync_replicas" {
  description = "Broker-side min.insync.replicas. Prod (RF3): 2 — one broker can be down and acks=all still commits. A 2-broker env MUST use 1, or a single broker outage stops all producing."
  type        = number
  default     = 2
}

variable "log_retention_hours" {
  description = "Default topic retention (hours). 168 = 7 days."
  type        = number
  default     = 168
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
