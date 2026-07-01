# infrastructure/modules/acm-cert/outputs.tf

output "certificate_arn" {
  description = "ARN of the validated (ISSUED) ACM certificate."
  # Reference the validation resource so consumers only get the ARN once the cert
  # is actually ISSUED (not merely created/pending).
  value = aws_acm_certificate_validation.cert.certificate_arn
}
