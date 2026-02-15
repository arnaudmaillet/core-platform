# infrastructure/live/staging/env.hcl

locals {
  env = "staging"

  # Networking
  vpc_cidr           = "10.20.0.0/16"
  single_nat_gateway = true # On peut encore économiser ici, sauf si test de charge intense

  # --- EKS Config ---
  # On utilise des instances "M" (General Purpose) pour avoir des perfs stables,
  # mais en taille "small" ou "medium" pour le staging.
  eks_instance_types_system   = ["t3.medium"]
  eks_instance_types_database = ["t3.large"] # On garde du large pour ScyllaDB/Postgres

  # Haute disponibilité : on veut tester le failover, donc min 2 nodes
  eks_min_size = 2
  eks_max_size = 5
}