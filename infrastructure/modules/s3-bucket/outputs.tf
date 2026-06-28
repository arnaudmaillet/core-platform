# infrastructure/modules/s3-bucket/outputs.tf

output "bucket_name" {
  description = "Bucket name (set as the service's *_BUCKET env)."
  value       = aws_s3_bucket.this.id
}

output "bucket_arn" {
  description = "Bucket ARN (scope the consumer's IRSA s3:* permissions to this + /*)."
  value       = aws_s3_bucket.this.arn
}

output "bucket_regional_domain_name" {
  description = "Regional domain name (for presign / CDN origin config)."
  value       = aws_s3_bucket.this.bucket_regional_domain_name
}
