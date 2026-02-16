# infrastructure/modules/networking/route53/main.tf

resource "aws_route53_zone" "main" {
  name          = "core-platform.click"
  force_destroy = false
}

output "zone_id" {
  description = "L'ID de la zone crée par AWS"
  value       = aws_route53_zone.main.zone_id
}