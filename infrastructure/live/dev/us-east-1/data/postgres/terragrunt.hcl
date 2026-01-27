# infrastructure/live/dev/us-east-1/data/postgres/terragrunt.hcl

include "root" {
  path = find_in_parent_folders("root.hcl")
}

terraform {
  source = "../../../../../modules/data/postgres"
}

dependency "vpc" {
  config_path = "../../networking"
}

inputs = {
  vpc_id             = dependency.vpc.outputs.vpc_id
  vpc_cidr           = dependency.vpc.outputs.vpc_cidr_block
  private_subnet_ids = dependency.vpc.outputs.private_data_subnet_ids

  db_name     = get_env("POSTGRES_DB", "identity_db")
  db_username = get_env("POSTGRES_USER", "postgres")
  db_password = get_env("POSTGRES_PASSWORD")
}