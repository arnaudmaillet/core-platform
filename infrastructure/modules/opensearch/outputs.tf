# infrastructure/modules/opensearch/outputs.tf

output "endpoint" {
  description = "VPC HTTPS endpoint (set as OPENSEARCH_URL, prefix with https://)."
  value       = aws_opensearch_domain.this.endpoint
}

output "arn" {
  description = "Domain ARN (used by the search IRSA policy to scope es:* access)."
  value       = aws_opensearch_domain.this.arn
}

output "master_secret_arn" {
  description = "Secrets Manager ARN holding the master username/password (synced to the app via External Secrets)."
  value       = aws_secretsmanager_secret.master.arn
}

output "security_group_id" {
  description = "Security group attached to the domain."
  value       = aws_security_group.opensearch.id
}
