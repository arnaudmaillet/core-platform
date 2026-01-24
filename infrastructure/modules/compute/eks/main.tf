module "eks" {
  source  = "terraform-aws-modules/eks/aws"
  version = "~> 19.0"

  cluster_name    = "${var.project_name}-${var.env}"
  cluster_version = "1.28" # Version stable de Kubernetes

  vpc_id     = var.vpc_id
  subnet_ids = var.private_subnets

  # Accès public à l'API (sécurisé par IP plus tard) et accès privé interne
  cluster_endpoint_public_access = true

  # EKS Managed Node Groups : AWS gère le cycle de vie des serveurs (patching, etc.)
  eks_managed_node_groups = {
    # Pool de base pour les services critiques (CoreDNS, Metrics Server)
    system = {
      min_size     = 2
      max_size     = 5
      desired_size = 2
      instance_types = ["t3.medium"]
    }
    # Pool "Spot" pour le Backend : 70% moins cher, idéal pour l'hyperscale
    apps = {
      min_size     = 2
      max_size     = 100 # Scalabilité massive
      desired_size = 5
      instance_types = ["t3.large", "c5.large"]
      capacity_type  = "SPOT"
    }
  }

  tags = {
    "k8s.io/cluster-autoscaler/enabled" = "true"
  }
}