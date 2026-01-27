# infrastructure/modules/kubernetes/eks/main.tf

module "eks" {
  source  = "terraform-aws-modules/eks/aws"
  version = "~> 20.0"

  cluster_name    = var.cluster_name
  cluster_version = "1.31"
  cluster_endpoint_public_access = true

  vpc_id     = var.vpc_id
  subnet_ids = var.private_subnet_ids

  # Hyperscale : Permettre au cluster de communiquer avec l'API AWS pour le scaling
  enable_cluster_creator_admin_permissions = true

  enable_irsa = true

  # --- Node Groups de Gestion (System) ---
  # Ces nœuds sont fixes et font tourner les composants vitaux du cluster
  eks_managed_node_groups = {
    system = {
      instance_types = ["t3.medium"]
      min_size       = 2
      max_size       = 4
      desired_size   = 2

      labels = {
        "intent" = "control-plane"
      }
    }
  }

  # --- Configuration Sécurité pour Karpenter ---
  # Karpenter a besoin de taguer les ressources qu'il crée dynamiquement
  node_security_group_tags = {
    "karpenter.sh/discovery" = var.cluster_name
  }
}

# --- Module Karpenter ---
# Autoscaling
module "karpenter" {
  source  = "terraform-aws-modules/eks/aws//modules/karpenter"
  version = "~> 20.0"

  cluster_name = module.eks.cluster_name

  # On utilise le rôle créé par le module EKS spécifiquement pour les nodes
  node_iam_role_name = module.eks.node_iam_role_name

  enable_v1_permissions = true
}