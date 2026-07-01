# infrastructure/modules/acm-cert/variables.tf

variable "domain_name" {
  type        = string
  description = "Apex domain for the cert; the wildcard SAN *.<domain> is added. Must match the account-global Route53 zone name."
  default     = "core-platform.click"
}
