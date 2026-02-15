# infrastructure/live/global/env.hcl
locals {
  env          = "global"
  project_name = basename(get_repo_root())
  owner        = basename(get_repo_root())

  # DNS Principal
  domain_name  = "core-platform.io"
}