# infrastructure/live/prod/us-east-1/networking/vpc/terragrunt.hcl
# Prod VPC: 3 AZs (env.hcl: 10.10.0.0/16, NAT-per-AZ via single_nat_gateway=false).

include "root" {
  path   = find_in_parent_folders("root.hcl")
  expose = true
}

terraform {
  source = "../../../../../modules/networking/vpc"
}

locals {
  env_vars    = read_terragrunt_config(find_in_parent_folders("env.hcl"))
  region_vars = read_terragrunt_config(find_in_parent_folders("region.hcl"))
}

inputs = {
  cluster_name       = "core-platform-${local.env_vars.locals.env}"
  vpc_cidr           = local.env_vars.locals.vpc_cidr
  availability_zones = ["${local.region_vars.locals.aws_region}a", "${local.region_vars.locals.aws_region}b", "${local.region_vars.locals.aws_region}c"]
  # env.hcl sets false: one NAT per AZ, so an AZ outage cannot sever egress.
  single_nat_gateway = local.env_vars.locals.single_nat_gateway
}
