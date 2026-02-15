# infrastructure/live/dev/us-east-1/data/postgres/terragrunt.hcl

include "root" {
  path = find_in_parent_folders("root.hcl")
}

terraform {
  source = "${get_repo_root()}//infrastructure/modules/data/postgres"
}

dependency "vpc" {
  config_path = "../../networking"
}

locals {
  # Charge le env.hcl le plus proche
  env_vars = read_terragrunt_config(find_in_parent_folders("env.hcl"))
}

inputs = {
  project_name = "core-platform"
  env          = local.env_vars.locals.env

  vpc_id             = dependency.vpc.outputs.vpc_id
  vpc_cidr           = dependency.vpc.outputs.vpc_cidr_block
  private_subnet_ids = dependency.vpc.outputs.private_data_subnet_ids

  # Configuration injectée depuis env.hcl
  instance_class          = local.env_vars.locals.db_instance_class
  allocated_storage       = local.env_vars.locals.db_allocated_storage
  multi_az                = local.env_vars.locals.db_multi_az
  deletion_protection     = local.env_vars.locals.db_deletion_protection
  apply_immediately       = local.env_vars.locals.db_apply_immediately
  backup_retention_period = local.env_vars.locals.db_backup_retention

  db_name     = "identity_db"
  db_username = "postgres"
}