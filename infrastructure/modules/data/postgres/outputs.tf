# infrastructure/modules/data/postgres/outputs.tf

output "db_instance_endpoint" {
  description = "L'adresse de connexion à la base de données"
  value       = aws_db_instance.this.endpoint
}

output "db_instance_address" {
  description = "L'hostname de la base de données (sans le port)"
  value       = aws_db_instance.this.address
}

output "db_instance_port" {
  description = "Le port de la base de données"
  value       = aws_db_instance.this.port
}

output "db_security_group_id" {
  value = aws_security_group.db.id
}

output "db_secret_arn" {
  value = aws_secretsmanager_secret.db_url.arn
}