include "root" {
  path = find_in_parent_folders("root.hcl")
}

terraform {
  source = "../../../../../modules/artifacts/ecr"
}

inputs = {
  project_name = "core-platform"
  env          = "dev"
}