# infrastructure/modules/acm-cert/main.tf
#
# The platform's public ACM certificate (apex + wildcard), DNS-validated against
# the account-global Route53 zone. Deliberately its OWN unit, DECOUPLED from eks:
# an ACM cert can't be deleted while an ALB/NLB still references it, and LB
# deprovision lags the k8s Service deletion. When the cert lived in the eks unit,
# that ResourceInUseException failed the eks destroy, which early-exited the vpc
# destroy → the whole VPC (NAT/subnets/ENIs) leaked. As a standalone unit with no
# dependency on eks/vpc, a cert-in-use failure is isolated: eks and vpc still
# destroy cleanly, and the cert is reclaimed on a later pass once the LB is gone.

data "aws_route53_zone" "main" {
  name         = var.domain_name
  private_zone = false
}

resource "aws_acm_certificate" "cert" {
  domain_name               = var.domain_name
  validation_method         = "DNS"
  subject_alternative_names = ["*.${var.domain_name}"]

  lifecycle {
    create_before_destroy = true
  }
}

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
