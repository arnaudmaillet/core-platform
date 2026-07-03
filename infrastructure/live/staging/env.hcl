# infrastructure/live/staging/env.hcl

locals {
  env = "staging"

  # Networking
  vpc_cidr           = "10.20.0.0/16"
  single_nat_gateway = true # On peut encore économiser ici, sauf si test de charge intense

  # --- EKS node groups (same shape as dev/env.hcl, consumed by modules/eks) ---
  # `system` for platform workloads; `database` (t3.large) for the stateful
  # in-cluster backends staging runs (ScyllaDB operator, CNPG) — pinned via the
  # `intent=database` label (nodeSelector) AND tainted dedicated=database, same
  # as the Karpenter database pool: without the taint, untargeted fleet pods can
  # spill onto these nodes and starve Scylla/CNPG. All 6 CNPG clusters + the
  # ScyllaCluster already tolerate it.
  node_groups = {
    system = {
      instance_types = ["t3.medium"]
      min_size       = 2
      max_size       = 3
      desired_size   = 2
      labels         = { intent = "system" }
      taints         = []

      iam_role_use_name_prefix = false
      iam_role_name            = "core-platform-staging-node-role"
    }
    database = {
      instance_types = ["t3.large"]
      min_size       = 2
      max_size       = 5
      desired_size   = 2
      labels         = { intent = "database" }
      taints = [{
        key    = "dedicated"
        value  = "database"
        effect = "NO_SCHEDULE"
      }]

      iam_role_use_name_prefix = false
      iam_role_name            = "core-platform-staging-db-node-role"
    }
  }
}
