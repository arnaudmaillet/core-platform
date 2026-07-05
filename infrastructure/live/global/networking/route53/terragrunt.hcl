# infrastructure/live/global/networking/route53/terragrunt.hcl
#
# Authoritative, account-global public hosted zone for the platform apex domain.
# Lives in `global/` (not per-env) because it maps to a single registrar
# delegation: exactly one zone for `core-platform.click` may be authoritative.
# Both clusters' EKS modules read it via `data "aws_route53_zone"` (no terragrunt
# dependency edge — they look it up by name), so it must exist before any env's
# `eks` unit applies. external-dns owns the per-env subdomain records at runtime.

include "root" {
  path = find_in_parent_folders("root.hcl")
}

terraform {
  # 4 levels up reaches infrastructure/ (this unit lives at
  # live/global/networking/route53). Relative paths keep terragrunt from copying
  # the whole repo as the module source (a get_repo_root()//… source would).
  source = "../../../../modules/networking/route53"
}

locals {
  env_vars = read_terragrunt_config(find_in_parent_folders("env.hcl"))
}

inputs = {
  # Single source of truth for the apex domain (global/env.hcl).
  domain_name = local.env_vars.locals.domain_name
}
