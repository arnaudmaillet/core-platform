# infrastructure/modules/kubernetes/eks/network_dns.tf

# --- RÉCUPÉRATION DE LA ZONE ROUTE 53 ---
data "aws_route53_zone" "main" {
  name         = "core-platform.click"
  private_zone = false
}

# --- CERTIFICAT SSL (ACM) ---
resource "aws_acm_certificate" "cert" {
  domain_name       = "core-platform.click"
  validation_method = "DNS"
  subject_alternative_names = ["*.core-platform.click"]

  lifecycle {
    create_before_destroy = true
  }
}

# --- VALIDATION DNS DU CERTIFICAT ---
resource "aws_route53_record" "cert_validation" {
  for_each = {
    for dvo in aws_acm_certificate.cert.domain_validation_options : dvo.domain_name => {
      name   = dvo.resource_record_name
      record = dvo.resource_record_value
      type   = dvo.resource_record_type
    }
  }

  allow_overwrite = true
  name            = each.value.name
  records         = [each.value.record]
  ttl             = 60
  type            = each.value.type
  zone_id         = data.aws_route53_zone.main.zone_id
}

resource "aws_acm_certificate_validation" "cert" {
  certificate_arn         = aws_acm_certificate.cert.arn
  validation_record_fqdns = [for record in aws_route53_record.cert_validation : record.fqdn]
}