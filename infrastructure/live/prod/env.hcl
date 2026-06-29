# ─────────────────────────────────────────────────────────────────────────────
# PROD SIZING INTENT — NOT YET WIRED.
#
# Prod has no live terragrunt units. The previous ones referenced a since-deleted
# module layout (modules/data/postgres, modules/kubernetes/eks) and were removed
# (they could never apply). This file is kept as the captured intent for when prod
# is actually stood up.
#
# To build prod, mirror the proven staging tree (infrastructure/live/staging/
# us-east-1/: region.hcl + networking/vpc + eks + security/irsa-roles +
# kubernetes/argocd + data/{msk,elasticache,opensearch,buckets,kms}) against the
# CURRENT modules, and RESHAPE the values below to what those modules expect:
#   * EKS wants a `node_groups` map (see staging/env.hcl), NOT eks_instance_types_*
#     / eks_min_size / eks_max_size.
#   * single_nat_gateway is now honored by modules/networking/vpc (false => one NAT
#     per AZ). vpc_cidr is consumed as-is.
# The good prod choices to carry forward: Graviton (m6g), NAT-per-AZ, min 3 nodes
# (survive one AZ), a distinct CIDR for peering.
# ─────────────────────────────────────────────────────────────────────────────
locals {
  env = "production"

  # Networking
  vpc_cidr           = "10.10.0.0/16" # CIDR différent pour éviter les conflits si peering
  single_nat_gateway = false          # Une NAT par AZ pour la haute disponibilité

  # EKS sizing intent (reshape to a `node_groups` map like staging/env.hcl).
  eks_instance_types_system   = ["m6g.medium"] # Graviton (meilleur ratio prix/perf)
  eks_instance_types_database = ["m6g.large"]
  eks_min_size                = 3 # Minimum 3 pour survivre à la perte d'une AZ
  eks_max_size                = 10
}