# infrastructure/modules/networking/route53/main.tf
#
# Authoritative public hosted zone for the platform apex domain. This is the SOLE
# creator of the zone — the eks/irsa-roles modules consume it read-only via
# `data "aws_route53_zone"` / `data "aws_acm_certificate"`, and external-dns
# writes per-env subdomain records (api.dev.*, *-staging.*, argocd.*, …) into it
# at runtime. It is account-global (one registrar delegation), so this unit lives
# in the `live/global` tree, NOT per-env — a second zone for the same apex would
# split the NS delegation and break resolution while staying green in Terraform.

variable "domain_name" {
  description = "Apex domain for the platform public hosted zone."
  type        = string
  default     = "core-platform.click"
}

resource "aws_route53_zone" "main" {
  name          = var.domain_name
  force_destroy = false
}

output "zone_id" {
  description = "L'ID de la zone crée par AWS"
  value       = aws_route53_zone.main.zone_id
}

output "name_servers" {
  description = "NS records to delegate at the registrar (only this zone's set is authoritative)."
  value       = aws_route53_zone.main.name_servers
}
