# infrastructure/live/dev/env.hcl

locals {
  env = "dev"

  # Networking
  vpc_cidr           = "10.0.0.0/16"
  single_nat_gateway = true 

  # EKS Node Groups : C'est ici qu'on définit la stratégie de nodes
  # En DEV : Un seul groupe "system" avec une Taint
  node_groups = {
    system = {
      instance_types = ["t3.medium"]
      min_size       = 2
      max_size       = 3
      desired_size   = 2
      labels         = { intent = "system" }
      taints         = []

      iam_role_use_name_prefix = false
      iam_role_name            = "core-platform-dev-node-role"
    }
  }
}