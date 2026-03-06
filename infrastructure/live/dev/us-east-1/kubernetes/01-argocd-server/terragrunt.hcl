# infrastructure/live/dev/us-east-1/kubernetes/01-argocd-server/terragrunt.hcl

include "root" {
  path = find_in_parent_folders("root.hcl")
}

dependency "eks" {
  config_path = "../../eks"
}

dependency "identity" {
  config_path = "../00-identity"
}

terraform {
  source = "../../../../../modules//kubernetes/argocd-server"
  
  before_hook "clean_k8s_resources" {
    commands     = ["destroy"]
    execute      = ["/bin/bash", "-c", "kubectl delete ingress --all --all-namespaces --wait=true || true"]
  }
}

inputs = {
  cluster_name           = dependency.eks.outputs.cluster_name
  cluster_endpoint       = dependency.eks.outputs.cluster_endpoint
  cluster_ca_certificate = dependency.eks.outputs.cluster_certificate_authority_data
}