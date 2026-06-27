# infrastructure/live/global/artifacts/ecr/terragrunt.hcl

include "root" {
  path = find_in_parent_folders("root.hcl")
}

terraform {
  source = "../../../../../modules/artifacts/ecr"
}

inputs = {
  # One ECR repo per fleet binary (the module prepends "core-platform-"), shared
  # across envs and tagged per env (dev/staging/latest). This is the authoritative
  # registry list — the overlays' `images:` newName must resolve to a repo here.
  service_names = [
    # ── Legacy / BFF ─────────────────────────────────────────────────────────
    "graphql-bff",
    "profile-command-server",
    # ── Shared tooling ───────────────────────────────────────────────────────
    "migrator",
    # BuildKit registry cache sink for the CI image builds (tag = BIN name).
    "buildcache",
    # ── Existing fleet (servers) ─────────────────────────────────────────────
    "chat-server",
    "social-graph-server",
    "profile-server",
    "geo-discovery-server",
    "notification-server",
    "post-server",
    "comment-server",
    "engagement-server",
    "account-server",
    "timeline-server",
    # ── New fleet (10 binaries) ──────────────────────────────────────────────
    "counter-server",
    "counter-worker",
    "audit-server",
    "audit-worker",
    "auth-server",
    "media-server",
    "moderation-server",
    "search-server",
    "realtime-gateway",
    "realtime-dispatcher",
  ]
}