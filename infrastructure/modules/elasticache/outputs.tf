# infrastructure/modules/elasticache/outputs.tf

output "configuration_endpoint" {
  description = "Cluster-mode configuration endpoint (set as REDIS_HOSTS)."
  value       = module.redis.replication_group_configuration_endpoint_address
}

output "port" {
  description = "Redis port."
  value       = module.redis.replication_group_port
}

output "auth_secret_arn" {
  description = "Secrets Manager ARN holding the Redis AUTH token (synced to the app via External Secrets)."
  value       = aws_secretsmanager_secret.auth.arn
}

output "security_group_id" {
  description = "Security group attached to the cache."
  value       = aws_security_group.redis.id
}
