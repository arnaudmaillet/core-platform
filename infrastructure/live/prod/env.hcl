# infrastructure/live/prod/env.hcl
#
# PROD environment knobs. The us-east-1 tree mirrors the proven staging layout
# (infrastructure/live/staging/us-east-1) with the production posture flipped on:
# 3 AZs everywhere, NAT-per-AZ, nothing disposable (no force_destroy, default
# secret recovery windows, COMPLIANCE-mode WORM), Graviton nodes.
#
# `env = "prod"` (not "production") on purpose: it is embedded in every resource
# name (core-platform-prod-*), the Terraform state bucket
# (core-platform-terraform-state-prod), the Secrets Manager names the overlay's
# ExternalSecrets reference literally, and the IRSA trust subjects
# ("default:prod-*") — short, and consistent with the k8s overlay's `prod-`
# namePrefix.
#
# BRING-UP PREREQUISITES (in order — see docs/runbooks/environment-lifecycle.md):
#   1. Create the state bucket core-platform-terraform-state-prod (root.hcl's
#      remote_state expects it).
#   2. Set endpoint_public_access_cidrs on the eks unit (admin/CI ranges) BEFORE
#      the first apply — prod must never expose the API to 0.0.0.0/0.
#   3. terragrunt run-all apply: vpc → eks → data → security → kubernetes/argocd.
#   4. Prod tracks `main`: merging develop → main is the deploy. The fleet-images
#      workflow pins the STAGING overlay only — a prod image-promotion workflow
#      is a tracked follow-up; until it exists the prod overlay's :prod tags are
#      placeholders that must be pinned by hand.

locals {
  env = "prod"

  # Networking — distinct CIDR from dev (10.0/16) and staging (10.20/16) so
  # future peering/VPN never collides. NAT-per-AZ: an AZ outage must not sever
  # egress (single NAT is the staging cost tradeoff, never prod's).
  vpc_cidr           = "10.10.0.0/16"
  single_nat_gateway = false

  # --- Admin / CI ingress allow-list ---
  # Locks the public EKS API endpoint (eks unit) and the ArgoCD/Grafana admin
  # ALBs to these ranges. Currently the admin workstation's egress IP
  # (2026-07-04). CI runner egress is deliberately NOT listed: the GitHub
  # workflows only push to git (fleet pin, prod-promote) and never touch the
  # EKS API or admin ALBs; Terraform applies run from the workstation. Add a
  # CIDR here if either of those facts changes, and update this entry when the
  # workstation IP rotates (a wrong value locks kubectl/ArgoCD out, not
  # Terraform — the AWS API is unaffected, so it is always recoverable by
  # editing this list and re-applying the eks unit).
  admin_cidrs = ["86.208.218.34/32"]

  # --- EKS managed node groups (consumed by modules/eks) ---
  # Graviton for price/perf (ami_type selects the ARM AL2023 AMI — required, the
  # module default is x86); min 3 so losing an AZ leaves quorum for system
  # workloads. The `database` group is tainted, same contract as staging and the
  # Karpenter database pool: only CNPG/Scylla/tolerating pods land there. Burst
  # and app capacity come from the Karpenter pools, not these groups.
  node_groups = {
    system = {
      instance_types = ["m6g.large"]
      min_size       = 3
      max_size       = 6
      desired_size   = 3
      labels         = { intent = "system" }
      taints         = []
      ami_type       = "AL2023_ARM_64_STANDARD"

      iam_role_use_name_prefix = false
      iam_role_name            = "core-platform-prod-node-role"
    }
    database = {
      # Memory-optimized: Scylla (8Gi/pod) + the 6 CNPG clusters live here.
      instance_types = ["r6g.large"]
      min_size       = 3
      max_size       = 6
      desired_size   = 3
      labels         = { intent = "database" }
      taints = [{
        key    = "dedicated"
        value  = "database"
        effect = "NO_SCHEDULE"
      }]
      ami_type = "AL2023_ARM_64_STANDARD"

      iam_role_use_name_prefix = false
      iam_role_name            = "core-platform-prod-db-node-role"
    }
  }
}
