# infrastructure/live/dev/us-east-1/kubernetes/addons/terragrunt.hcl

include "root" { path = find_in_parent_folders("root.hcl") }

terraform {
  source = "../../../../../modules/kubernetes/addons"
}

# Dépendance cruciale : Addons a besoin des outputs de EKS
dependency "eks" {
  config_path = "../eks"
}

dependency "vpc" {
  config_path = "../../networking/vpc"
}

dependency "route53" {
  config_path = "../../networking/route53"
}

inputs = {
  # Branchement automatique des outputs de EKS vers les variables de Addons
  cluster_name           = dependency.eks.outputs.cluster_name
  cluster_endpoint       = dependency.eks.outputs.cluster_endpoint
  cluster_ca_certificate = dependency.eks.outputs.cluster_certificate_authority_data
  vpc_id                 = dependency.vpc.outputs.vpc_id

  # Rôles IAM et Certificat
  lb_controller_role_arn   = dependency.eks.outputs.lb_controller_role_arn
  ebs_csi_role_arn         = dependency.eks.outputs.ebs_csi_role_arn
  eks_oidc_provider_arn = dependency.eks.outputs.oidc_provider_arn
  route53_zone_id = dependency.route53.outputs.zone_id
  ssl_certificate_arn      = dependency.eks.outputs.ssl_certificate_arn
  karpenter_node_role_name = dependency.eks.outputs.karpenter_node_role_name
  external_dns_role_arn         = dependency.eks.outputs.external_dns_role_arn
  karpenter_controller_role_arn = dependency.eks.outputs.karpenter_controller_role_arn
  create_karpenter_node_iam_role = false
}