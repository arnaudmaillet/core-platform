# infrastructure/modules/msk/outputs.tf

output "bootstrap_brokers_sasl_scram" {
  description = "SASL/SCRAM bootstrap brokers (set as KAFKA_BROKERS)."
  value       = module.msk.bootstrap_brokers_sasl_scram
}

output "scram_secret_arn" {
  description = "Secrets Manager ARN holding the SCRAM username/password (synced to the app via External Secrets)."
  value       = aws_secretsmanager_secret.scram.arn
}

output "broker_security_group_id" {
  description = "Security group attached to the brokers."
  value       = aws_security_group.broker.id
}
