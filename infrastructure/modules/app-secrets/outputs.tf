# infrastructure/modules/app-secrets/outputs.tf

output "media_s3_secret_arn" {
  description = "ARN of the media static-key secret."
  value       = aws_secretsmanager_secret.media_s3.arn
}

output "scylla_s3_secret_arn" {
  description = "ARN of the scylla-manager-agent backup static-key secret."
  value       = aws_secretsmanager_secret.scylla_s3.arn
}

output "audit_crypto_secret_arn" {
  description = "ARN of the audit crypto/custody secret."
  value       = aws_secretsmanager_secret.audit_crypto.arn
}

output "auth_secrets_secret_arn" {
  description = "ARN of the auth signing/Keycloak secret."
  value       = aws_secretsmanager_secret.auth_secrets.arn
}
