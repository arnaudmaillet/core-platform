# infrastructure/live/dev/us-east-1/kubernetes/03-argocd-addons/terragrunt.hcl

include "root" {
  path = find_in_parent_folders("root.hcl")
}

terraform {
  source = "../../../../../modules//kubernetes/argocd-addons"
}

dependency "eks" {
  config_path = "../../eks"
}

dependency "identity" {
  config_path = "../identity"
}

dependency "argocd_server" {
  config_path = "../01-argocd-server"
}

dependency "argocd_server" {
  config_path = "../02-argocd-bootstrap"
}

inputs = {
  cluster_name = dependency.eks.outputs.cluster_name
  
  addons = {
    aws-ebs-csi-driver = {
      service_account_role_arn = dependency.identity.outputs.iam_role_arns.ebs_csi
    }
  }

  tags = {
    Environment = "dev"
    ManagedBy   = "terragrunt"
  }
}