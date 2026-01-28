# infrastructure/live/dev/us-east-1/artifacts/ecr/terragrunt.hcl

include "root" {
  path = find_in_parent_folders("root.hcl")
}

terraform {
  source = "${get_repo_root()}//infrastructure/modules/artifacts/ecr"
}

inputs = {
  project_name = "core-platform"
  env          = "dev"
}