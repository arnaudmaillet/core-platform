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
      # t3.large, not t3.medium: the medium's kubelet max-pods cap (17) saturates
      # on platform pods alone — on the 2026-07-04 rebuild both system nodes hit
      # the cap and the node-exporter DaemonSet pods were unschedulable ("Too many
      # pods") until pods were manually evicted. t3.large lifts the cap to 35.
      # Prefix delegation is now enabled cluster-wide (modules/eks vpc-cni
      # addon, before_compute) so max-pods no longer binds at 17 even on
      # mediums; t3.large is kept anyway — the saturation was also a CPU/mem
      # headroom signal, not just a pod-slot one.
      instance_types = ["t3.large"]
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
      # 3 nodes: one per Scylla member — the fleet's keyspaces are created at
      # NetworkTopologyStrategy RF 3 (per-service migrations) and every write
      # is LOCAL_QUORUM, so fewer than 2 live members fails ALL writes (found
      # live: the first soak's 6,850 CreatePost calls all failed on a 1-node
      # Scylla). CNPG's 12 instances also breathe easier across 3 nodes.
      min_size     = 3
      max_size     = 5
      desired_size = 3
      labels       = { intent = "database" }
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
