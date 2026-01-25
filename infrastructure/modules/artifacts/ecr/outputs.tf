output "repository_url" {
  description = "URL du repository ECR"
  value       = aws_ecr_repository.main.repository_url
}

output "repository_arn" {
  description = "ARN du repository pour les politiques IAM"
  value       = aws_ecr_repository.main.arn
}