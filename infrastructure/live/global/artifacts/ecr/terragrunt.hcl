# infrastructure/live/global/artifacts/ecr/terragrunt.hcl

include "root" {
  path = find_in_parent_folders("root.hcl")
}

terraform {
  # 4 levels up reaches infrastructure/ (this unit lives at live/global/artifacts/ecr).
  # NB: relative paths keep terragrunt from copying the whole repo as the module
  # source (a get_repo_root()//… source would).
  source = "../../../../modules/artifacts/ecr"
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
    # Kafka topic provisioning from the event-topology registry (PreSync Job).
    "topic-provisioner",
    # NB: no "buildcache" repo. The BuildKit cook cache lives on GHCR, not ECR —
    # the runners are on GitHub, so an ECR-hosted cache billed egress on every
    # pull (1,028 GB / 83.55 USD in July 2026, the account's largest line). It was
    # also the one repo the shared `imageCountMoreThan: 30` lifecycle rule broke:
    # 44 cache tags in a single repo meant 14 were always expiring in rotation.
    # See .github/workflows/fleet-images-deploy.yml.
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
    "media-worker",
    "moderation-server",
    "search-server",
    "realtime-gateway",
    "realtime-dispatcher",
  ]
}