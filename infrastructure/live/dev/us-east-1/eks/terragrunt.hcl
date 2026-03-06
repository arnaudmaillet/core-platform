# infrastructure/live/dev/us-east-1/eks/terragrunt.hcl

include "root" {
  path = find_in_parent_folders("root.hcl")
}

terraform {
  source = "../../../../modules/eks"

  # --- NETTOYAGE PRÉ-DESTROY ---
  # On s'assure que les ressources critiques (Ingress/ALB) sont parties
  # AVANT que Terraform ne commence à supprimer les Nodes EKS.
  before_hook "k8s_cleanup" {
    commands     = ["destroy"]
    # On supprime tous les ingress. Le --wait=true est vital ici.
    # On ajoute || true pour ne pas bloquer le destroy si le cluster est déjà partiellement cassé.
    execute      = ["/bin/bash", "-c", "kubectl delete ingress --all --all-namespaces --timeout=5m || true"]
  }

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