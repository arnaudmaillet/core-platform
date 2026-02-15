# infrastructure/live/global/artifacts/ecr/terragrunt.hcl

include "root" {
  path = find_in_parent_folders("root.hcl")
}

terraform {
  source = "../../../../../modules/artifacts/ecr"
}

inputs = {
  service_names = [
    "graphql-bff",
    "profile-command-server",
  ]
}