# infrastructure/modules/kubernetes/eks/iam.tf

# --- IAM POUR LOAD BALANCER CONTROLLER ---
resource "aws_iam_policy" "load_balancer_controller" {
  name   = "${var.cluster_name}-lb-controller-policy"
  policy = var.iam_policy_json_content
}

module "lb_controller_irsa_role" {
  source  = "terraform-aws-modules/iam/aws//modules/iam-role-for-service-accounts-eks"
  version = "~> 5.0"

  role_name = "${var.cluster_name}-lb-controller-role"
  role_policy_arns = {
    policy = aws_iam_policy.load_balancer_controller.arn
  }

  oidc_providers = {
    main = {
      provider_arn               = module.eks.oidc_provider_arn
      namespace_service_accounts = ["kube-system:aws-load-balancer-controller"]
    }
  }
}

# --- IAM POUR EXTERNAL-DNS ---
module "external_dns_irsa_role" {
  source  = "terraform-aws-modules/iam/aws//modules/iam-role-for-service-accounts-eks"
  version = "~> 5.0"

  role_name                     = "${var.cluster_name}-external-dns-role"
  attach_external_dns_policy    = true
  external_dns_hosted_zone_arns = [data.aws_route53_zone.main.arn]

  oidc_providers = {
    main = {
      provider_arn               = module.eks.oidc_provider_arn
      namespace_service_accounts = ["kube-system:external-dns"]
    }
  }
}

# --- IAM POUR KARPENTER (Controller) ---
module "karpenter_irsa_role" {
  source  = "terraform-aws-modules/iam/aws//modules/iam-role-for-service-accounts-eks"
  version = "~> 5.0"

  role_name                          = "${var.cluster_name}-karpenter-controller-role"
  attach_karpenter_controller_policy = true

  karpenter_controller_cluster_name       = module.eks.cluster_name
  karpenter_controller_node_iam_role_arns = [
    module.eks.eks_managed_node_groups["system"].iam_role_arn
  ]

  oidc_providers = {
    main = {
      provider_arn               = module.eks.oidc_provider_arn
      namespace_service_accounts = ["kube-system:karpenter"]
    }
  }
}

resource "aws_iam_role_policy" "karpenter_controller_extra" {
  name = "KarpenterControllerExtraPermissions"
  role = module.karpenter_irsa_role.iam_role_name

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Action = [
          "iam:GetInstanceProfile",
          "iam:CreateInstanceProfile",
          "iam:TagInstanceProfile",
          "iam:AddRoleToInstanceProfile",
          "iam:RemoveRoleFromInstanceProfile",
          "iam:DeleteInstanceProfile"
        ]
        Effect   = "Allow"
        Resource = "*"
      }
    ]
  })
}

# --- IAM POUR EBS (Disques) ---
module "ebs_csi_irsa_role" {
  source  = "terraform-aws-modules/iam/aws//modules/iam-role-for-service-accounts-eks"
  version = "~> 5.0"

  role_name             = "${var.cluster_name}-ebs-csi-role"
  attach_ebs_csi_policy = true

  oidc_providers = {
    main = {
      provider_arn               = module.eks.oidc_provider_arn
      namespace_service_accounts = ["kube-system:ebs-csi-controller-sa"]
    }
  }
}