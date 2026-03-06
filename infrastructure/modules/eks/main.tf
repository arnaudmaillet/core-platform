# infrastructure/modules/eks/main.tf

# --- MODULE EKS PRINCIPAL ---
module "eks" {
  source  = "terraform-aws-modules/eks/aws"
  version = "~> 20.0"

  cluster_name    = var.cluster_name
  cluster_version = "1.31"

  cluster_endpoint_public_access = true

  vpc_id     = var.vpc_id
  subnet_ids = var.private_subnet_ids

  enable_cluster_creator_admin_permissions = true
  enable_irsa                              = true

# --- MANAGED NODE GROUPS DYNAMIQUES ---
  # Ici, on passe directement la map configurée dans Terragrunt.
  # Cela permet de varier le nombre et le type de groupes selon l'environnement.
  eks_managed_node_groups = var.node_groups
  
  # Tag crucial pour que Karpenter identifie le Security Group à utiliser pour les nouveaux nodes
  node_security_group_tags = {
    "karpenter.sh/discovery" = var.cluster_name
  }

  node_security_group_additional_rules = {
    ingress_argocd_8080 = {
      description = "Allow ALB to reach ArgoCD server pods"
      protocol    = "tcp"
      from_port   = 8080
      to_port     = 8080
      type        = "ingress"
      # On autorise le trafic venant de l'ALB (via le CIDR du VPC)
      cidr_blocks = ["10.0.0.0/16"] # Utilise ton CIDR VPC ici
    }
  }
}