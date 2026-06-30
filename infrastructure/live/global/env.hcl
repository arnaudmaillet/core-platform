# infrastructure/live/global/env.hcl
locals {
  env          = "global"
  project_name = basename(get_repo_root())
  owner        = basename(get_repo_root())

  # DNS Principal — the platform apex domain. This is the single source of truth;
  # it MUST match the domain hardcoded in the eks/irsa-roles data lookups and the
  # ingress/cert-manager/external-dns config (k8s/ + infrastructure/argocd/).
  domain_name = "core-platform.click"
}