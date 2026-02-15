locals {
  env = "production"

  # Networking
  vpc_cidr           = "10.10.0.0/16" # CIDR différent pour éviter les conflits si peering
  single_nat_gateway = false # Une NAT par AZ pour la haute disponibilité

  # EKS Config
  eks_instance_types_system   = ["m6g.medium"] # Graviton (meilleur ratio prix/perf)
  eks_instance_types_database = ["m6g.large"]
  eks_min_size                = 3 # Minimum 3 pour survivre à la perte d'une AZ
  eks_max_size                = 10
}