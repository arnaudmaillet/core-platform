# infrastructure/live/prod/us-east-1/networking/acm-cert/terragrunt.hcl
#
# Public ACM cert (apex + wildcard), DNS-validated against the account-global
# Route53 zone. Standalone/decoupled from eks so a cert-in-use failure at teardown
# (LB still deprovisioning) can't early-exit the vpc destroy and leak the VPC.
# No dependencies: it only data-reads the global zone. Consumed by kubernetes/argocd.

include "root" {
  path = find_in_parent_folders("root.hcl")
}

terraform {
  source = "../../../../../modules/acm-cert"
}

# domain_name defaults to core-platform.click (the account-global zone).
