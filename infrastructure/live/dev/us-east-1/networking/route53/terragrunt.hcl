# infrastructure/live/dev/us-east-1/networking/route53/terragrunt.hcl

include "root" { path = find_in_parent_folders("root.hcl") }

terraform {
  source = "../../../../../modules/networking/route53"
}