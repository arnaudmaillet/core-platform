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
