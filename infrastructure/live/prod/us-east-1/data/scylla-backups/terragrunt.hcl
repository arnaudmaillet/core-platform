# infrastructure/live/prod/us-east-1/data/scylla-backups/terragrunt.hcl
#
# Scylla Manager backup sink. Scylla is the system of record for the majority of
# user data (chat/post/comment/social-graph/timeline/notification) and previously
# had NO backup story at all — CNPG had PITR, but a lost Scylla volume was
# unrecoverable. Scylla Manager (operators appset) snapshots into this bucket per
# the ScyllaCluster's `spec.backups` task; retention is enforced by the manager
# (it purges old snapshots), so no bucket lifecycle rule and no versioning.
#
# The agent authenticates with static keys from the app-secrets unit
# (<name>-scylla-s3), synced into the `scylla` namespace by an ExternalSecret —
# the same v1 static-key posture as media/audit (IRSA migration is the tracked
# deferral; the scylla-manager-agent's rclone can't assume a web identity on the
# member pods' SA today without operator support).

include "root" {
  path = find_in_parent_folders("root.hcl")
}

terraform {
  source = "../../../../../modules/s3-bucket"
}

locals {
  env_vars   = read_terragrunt_config(find_in_parent_folders("env.hcl"))
  account_id = get_aws_account_id()
}

inputs = {
  name = "core-platform-${local.env_vars.locals.env}-scylla-backups-${local.account_id}"

  # Manager owns retention (spec.backups[].retention) — versioning would keep
  # every purged snapshot forever and defeat it.
  versioning_enabled = false

  # Prod backups are the recovery story — destroy must never empty this bucket.
  force_destroy = false

  tags = {
    Environment = local.env_vars.locals.env
    ManagedBy   = "terragrunt"
  }
}
