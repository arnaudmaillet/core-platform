# infrastructure/live/dev/us-east-1/eks/terragrunt.hcl

include "root" {
  path = find_in_parent_folders("root.hcl")
}

terraform {
  source = "../../../../modules/eks"

  # Le hook peut être conservé pour le debug, mais il est moins critique 
  # maintenant que le destroy ne bloque plus sur les ressources K8s.
  after_hook "cleanup_check" {
    commands     = ["destroy"]
    execute      = ["/bin/bash", "-c", "echo 'Vérification finale des ressources orphelines...'; sleep 5"]
    run_on_error = false
  }
}

dependency "vpc" {
  config_path = "../networking/vpc"
}

locals {
  env_vars = read_terragrunt_config(find_in_parent_folders("env.hcl"))
}

inputs = {
  cluster_name       = "core-platform-${local.env_vars.locals.env}"
  vpc_id             = dependency.vpc.outputs.vpc_id
  private_subnet_ids = dependency.vpc.outputs.private_app_subnet_ids

  # Injecte la map dynamique définie dans env.hcl (Node Groups système)
  node_groups = local.env_vars.locals.node_groups

  project_name = "core-platform"
  env          = local.env_vars.locals.env
}