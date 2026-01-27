# infrastructure/modules/artifacts/ecr/outputs.tf

output "repository_urls" {
  description = "Map des URLs des repositories ECR"
  value       = { for k, v in aws_ecr_repository.services : k => v.repository_url }
}

output "repository_arns" {
  description = "Map des ARNs des repositories ECR"
  value       = { for k, v in aws_ecr_repository.services : k => v.arn }
}