# infrastructure/modules/kms-key/outputs.tf

output "key_arn" {
  description = "KMS key ARN (scope the consumer's IRSA kms:* permissions to this)."
  value       = aws_kms_key.this.arn
}

output "key_id" {
  description = "KMS key id."
  value       = aws_kms_key.this.key_id
}

output "alias_name" {
  description = "Alias name (alias/<alias>)."
  value       = aws_kms_alias.this.name
}
