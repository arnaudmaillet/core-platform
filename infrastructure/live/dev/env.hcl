# infrastructure/live/dev/env.hcl

locals {
  env = "dev"

  # Networking
  vpc_cidr           = "10.0.0.0/16"
  single_nat_gateway = true # Une seule NAT pour économiser ~60$/mois

  # EKS Node Groups Settings
  system_node_settings = {
    instance_types = ["t3.medium"]
    min_size       = 2
    max_size       = 3
    desired_size   = 2
  }

  mgmt_node_settings = {
    instance_types = ["t3.large"]
    min_size       = 1
    max_size       = 3
    desired_size   = 1
  }

  db_node_settings = {
    instance_types = ["r6i.large"]
    min_size       = 1
    max_size       = 3
    desired_size   = 1
  }
}