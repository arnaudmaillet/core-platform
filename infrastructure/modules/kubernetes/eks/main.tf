# infrastructure/modules/kubernetes/eks/main.tf

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

  # Groupes de machines (Managed Node Groups)
  eks_managed_node_groups = {

    system = {
      instance_types = var.system_node_settings.instance_types
      min_size       = var.system_node_settings.min_size
      max_size       = var.system_node_settings.max_size
      desired_size   = var.system_node_settings.desired_size
      labels         = { "intent" = "system" }
      iam_role_use_name_prefix = false
      iam_role_name            = "${var.cluster_name}-node-role"
    }

    management = {
      instance_types = var.mgmt_node_settings.instance_types
      min_size       = var.mgmt_node_settings.min_size
      max_size       = var.mgmt_node_settings.max_size
      desired_size   = var.mgmt_node_settings.desired_size
      labels         = { "intent" = "management" }
    }

    database = {
      instance_types = var.db_node_settings.instance_types
      min_size       = var.db_node_settings.min_size
      max_size       = var.db_node_settings.max_size
      desired_size   = var.db_node_settings.desired_size

      taints = [{
        key    = "dedicated"
        value  = "database"
        effect = "NO_SCHEDULE"
      }]

      iam_role_additional_policies = {
        AmazonEBSCSIDriverPolicy = "arn:aws:iam::aws:policy/service-role/AmazonEBSCSIDriverPolicy"
      }
      labels = { "role" = "storage" }
    }
  }

  node_security_group_tags = {
    "karpenter.sh/discovery" = var.cluster_name
  }
}