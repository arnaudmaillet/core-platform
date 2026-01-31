# infrastructure/modules/kubernetes/eks/main.tf

# ---------------------------------------------------------------------------------------------------------------------
# CLUSTER EKS
# ---------------------------------------------------------------------------------------------------------------------
module "eks" {
  source  = "terraform-aws-modules/eks/aws"
  version = "~> 20.0"

  cluster_name    = var.cluster_name
  cluster_version = "1.31"
  cluster_endpoint_public_access = true

  vpc_id     = var.vpc_id
  subnet_ids = var.private_subnet_ids

  # Accès admin pour le créateur du cluster (indispensable pour gérer les ressources K8s ensuite)
  enable_cluster_creator_admin_permissions = true
  enable_irsa = true

  # --- Add-ons du Cluster ---
  # L'Amazon EBS CSI Driver permet à l'Operator Scylla de créer des volumes EBS
  cluster_addons = {
    aws-ebs-csi-driver = {
      most_recent = true
    }
  }

  # --- Node Groups managés ---
  eks_managed_node_groups = {
    system = {
      instance_types = ["t3.medium"]
      min_size       = 2
      max_size       = 4
      desired_size   = 2

      # Ajout de la policy IAM nécessaire pour que les nodes manipulent les disques EBS
      iam_role_additional_policies = {
        AmazonEBSCSIDriverPolicy = "arn:aws:iam::aws:policy/service-role/AmazonEBSCSIDriverPolicy"
      }

      labels = {
        "intent" = "control-plane"
      }
    }
  }

  # Tags pour permettre la découverte du cluster par Karpenter
  node_security_group_tags = {
    "karpenter.sh/discovery" = var.cluster_name
  }
}

# ---------------------------------------------------------------------------------------------------------------------
# AUTOSCALING (KARPENTER)
# ---------------------------------------------------------------------------------------------------------------------
module "karpenter" {
  source  = "terraform-aws-modules/eks/aws//modules/karpenter"
  version = "~> 20.0"

  cluster_name = module.eks.cluster_name

  node_iam_role_name    = module.eks.node_iam_role_name
  enable_v1_permissions = true
}

# ---------------------------------------------------------------------------------------------------------------------
# STOCKAGE PAR DÉFAUT (GP3)
# ---------------------------------------------------------------------------------------------------------------------
resource "kubernetes_storage_class_v1" "gp3" {
  metadata {
    name = "gp3"
    annotations = {
      # On définit gp3 comme stockage par défaut pour tout le cluster
      "storageclass.kubernetes.io/is-default-class" = "true"
    }
  }
  storage_provisioner    = "ebs.csi.aws.com"
  reclaim_policy         = "Delete"
  allow_volume_expansion = true
  volume_binding_mode    = "WaitForFirstConsumer"
  parameters = {
    type      = "gp3"
    encrypted = "true"
  }

  depends_on = [module.eks]
}

# ---------------------------------------------------------------------------------------------------------------------
# CERT-MANAGER (Requis pour les Webhooks de l'Operator Scylla)
# ---------------------------------------------------------------------------------------------------------------------
resource "helm_release" "cert_manager" {
  name             = "cert-manager"
  repository       = "https://charts.jetstack.io"
  chart            = "cert-manager"
  version          = "v1.13.0"
  namespace        = "cert-manager"
  create_namespace = true

  set = [
    {
      name  = "installCRDs"
      value = "true"
    }
  ]

  depends_on = [module.eks]
}

# ---------------------------------------------------------------------------------------------------------------------
# SCYLLADB OPERATOR
# ---------------------------------------------------------------------------------------------------------------------
resource "helm_release" "scylla_operator" {
  name             = "scylla-operator"
  repository       = "https://scylla-operator-charts.storage.googleapis.com/stable"
  chart            = "scylla-operator"
  namespace        = "scylla-operator"
  create_namespace = true

  depends_on = [helm_release.cert_manager]
}


# ---------------------------------------------------------------------------------------------------------------------
# Configuration des Providers (Bootstrap local au module)
# ---------------------------------------------------------------------------------------------------------------------

data "aws_eks_cluster_auth" "cluster" {
  name = module.eks.cluster_name
}

provider "kubernetes" {
  host                   = module.eks.cluster_endpoint
  cluster_ca_certificate = base64decode(module.eks.cluster_certificate_authority_data)
  token                  = data.aws_eks_cluster_auth.cluster.token
}

provider "helm" {
  kubernetes = {
    host                   = module.eks.cluster_endpoint
    cluster_ca_certificate = base64decode(module.eks.cluster_certificate_authority_data)
    token                  = data.aws_eks_cluster_auth.cluster.token
  }
}