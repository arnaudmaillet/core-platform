# infrastructure/live/dev/us-east-1/kubernetes/addons/core/terragrunt.hcl

include "root" { path = find_in_parent_folders("root.hcl") }

terraform {
  # On pointe vers le même module, mais on filtrera les ressources à activer
  source = "../../../../../../modules/kubernetes/addons/core"
}

dependency "eks" { config_path = "../../eks" }
dependency "vpc" { config_path = "../../../networking/vpc" }

inputs = {
  cluster_name           = dependency.eks.outputs.cluster_name
  cluster_endpoint       = dependency.eks.outputs.cluster_endpoint
  cluster_ca_certificate = dependency.eks.outputs.cluster_certificate_authority_data
  vpc_id                 = dependency.vpc.outputs.vpc_id

  # Rôles IAM spécifiques au Core
  lb_controller_role_arn        = dependency.eks.outputs.lb_controller_role_arn
  ebs_csi_role_arn              = dependency.eks.outputs.ebs_csi_role_arn
  karpenter_node_role_name      = dependency.eks.outputs.karpenter_node_role_name
  karpenter_controller_role_arn = dependency.eks.outputs.karpenter_controller_role_arn
  eks_oidc_provider_arn         = dependency.eks.outputs.oidc_provider_arn

  # Flags pour n'activer QUE le Core (à adapter selon tes variables de module)
  enable_core_addons    = true
  enable_apps_addons    = false
}