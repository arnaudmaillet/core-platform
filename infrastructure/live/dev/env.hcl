# infrastructure/live/dev/env.hcl

locals {
  env = "dev"

  # Networking
  vpc_cidr           = "10.0.0.0/16"
  single_nat_gateway = true # Une seule NAT pour économiser ~60$/mois

  # EKS Config
  eks_instance_types_system   = ["t3.medium"]
  eks_instance_types_database = ["t3.large"]
  eks_desired_size = 5
  eks_min_size = 1
  eks_max_size = 10
}