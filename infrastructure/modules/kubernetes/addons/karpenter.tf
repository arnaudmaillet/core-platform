# infrastructure/modules/kubernetes/addons/karpenter.tf

# Récupère dynamiquement ton ID de compte AWS
data "aws_caller_identity" "current" {}

module "karpenter" {
  source  = "terraform-aws-modules/eks/aws//modules/karpenter"
  version = "~> 20.0"

  cluster_name = var.cluster_name
  enable_v1_permissions = true

  enable_irsa = true
  irsa_oidc_provider_arn = var.eks_oidc_provider_arn
  irsa_namespace_service_accounts = ["kube-system:karpenter"]

  create_node_iam_role = false

  # On passe le NOM (déjà fait)
  node_iam_role_name   = var.karpenter_node_role_name

  # AJOUTE CETTE LIGNE : On passe l'ARN complet
  # On le reconstruit dynamiquement pour éviter de modifier les outputs de EKS
  node_iam_role_arn    = "arn:aws:iam::${data.aws_caller_identity.current.account_id}:role/${var.karpenter_node_role_name}"
  create_access_entry = false
}