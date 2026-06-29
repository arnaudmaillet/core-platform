# infrastructure/modules/kubernetes/identity/main.tf

# --- CONFIGURATION DU PROVIDER AWS ---
# Ce module ne nécessite QUE le provider AWS. 
# Aucun lien avec l'API Kubernetes ici pour éviter les blocages au destroy.

# --- 1. AWS LOAD BALANCER CONTROLLER ---
# On crée d'abord la policy spécifique (nécessite var.iam_policy_json_content)
resource "aws_iam_policy" "load_balancer_controller" {
  name   = "${var.cluster_name}-lb-controller-policy"
  policy = var.iam_policy_json_content
}

module "lb_controller_irsa_role" {
  source  = "terraform-aws-modules/iam/aws//modules/iam-role-for-service-accounts-eks"
  version = "~> 5.0"

  role_name = "${var.cluster_name}-lb-controller-role"

  # On attache ta policy personnalisée
  role_policy_arns = {
    policy = aws_iam_policy.load_balancer_controller.arn
  }

  oidc_providers = {
    main = {
      provider_arn               = var.oidc_provider_arn
      namespace_service_accounts = ["kube-system:aws-load-balancer-controller"]
    }
  }
}

# --- 2. EXTERNAL-DNS ---
module "external_dns_irsa_role" {
  source  = "terraform-aws-modules/iam/aws//modules/iam-role-for-service-accounts-eks"
  version = "~> 5.0"

  role_name                     = "${var.cluster_name}-external-dns-role"
  attach_external_dns_policy    = true
  external_dns_hosted_zone_arns = ["arn:aws:route53:::hostedzone/*"]

  oidc_providers = {
    main = {
      provider_arn               = var.oidc_provider_arn
      namespace_service_accounts = ["external-dns:external-dns"]
    }
  }
}

# Permission extra pour lister les zones (souvent oubliée par le module de base)
resource "aws_iam_role_policy" "external_dns_list_zones" {
  name = "ExternalDNSListZones"
  role = module.external_dns_irsa_role.iam_role_name

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Action   = ["route53:ListHostedZones", "route53:ListResourceRecordSets"]
        Effect   = "Allow"
        Resource = "*"
      }
    ]
  })
}

# --- 3. KARPENTER (Controller) ---
module "karpenter_irsa_role" {
  source  = "terraform-aws-modules/iam/aws//modules/iam-role-for-service-accounts-eks"
  version = "~> 5.0"

  role_name                          = "${var.cluster_name}-karpenter-controller-role"
  attach_karpenter_controller_policy = true
  karpenter_controller_cluster_name  = var.cluster_name

  oidc_providers = {
    main = {
      provider_arn               = var.oidc_provider_arn
      namespace_service_accounts = ["karpenter:karpenter"]
    }
  }
}

# Tes permissions cruciales pour Karpenter (RunInstances, PassRole, etc.)
resource "aws_iam_role_policy" "karpenter_controller_extra" {
  name = "KarpenterControllerExtraPermissions"
  role = module.karpenter_irsa_role.iam_role_name

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Action = [
          "ec2:RunInstances",
          "ec2:TerminateInstances",
          "ec2:CreateFleet",
          "ec2:CreateLaunchTemplate",
          "ec2:CreateTags",
          "iam:GetInstanceProfile",
          "iam:CreateInstanceProfile",
          "iam:CreateServiceLinkedRole",
          "iam:TagInstanceProfile",
          "iam:AddRoleToInstanceProfile",
          "iam:RemoveRoleFromInstanceProfile",
          "iam:DeleteInstanceProfile",
          "ec2:DeleteLaunchTemplate",
          "ec2:DescribeLaunchTemplates",
          "ec2:DescribeSubnets",
          "ec2:DescribeSecurityGroups",
          "ec2:DescribeInstances",
          "ec2:DescribeInstanceTypes",
          "ec2:DescribeInstanceTypeOfferings",
          "ec2:DescribeAvailabilityZones",
          "ec2:DescribeImages",
          "ec2:DescribeSpotPriceHistory",
          "ssm:GetParameter"
        ]
        Effect   = "Allow"
        Resource = "*"
      },
      {
        Action   = "iam:PassRole"
        Effect   = "Allow"
        Resource = var.node_iam_role_arns
      }
    ]
  })
}

resource "aws_iam_role_policy" "external_dns_route53_rw" {
  name = "ExternalDNSRoute53ReadWrite"
  role = module.external_dns_irsa_role.iam_role_name

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Action = [
          "route53:ChangeResourceRecordSets",
          "route53:ListHostedZones",
          "route53:ListResourceRecordSets"
        ]
        Effect   = "Allow"
        Resource = "*"
      }
    ]
  })
}

# --- 4. EBS CSI DRIVER ---
module "ebs_csi_irsa_role" {
  source  = "terraform-aws-modules/iam/aws//modules/iam-role-for-service-accounts-eks"
  version = "~> 5.0"

  role_name             = "${var.cluster_name}-ebs-csi-role"
  attach_ebs_csi_policy = true

  oidc_providers = {
    main = {
      provider_arn               = var.oidc_provider_arn
      namespace_service_accounts = ["kube-system:ebs-csi-controller-sa"]
    }
  }
}

# --- 5. CERT-MANAGER ---
module "cert_manager_irsa_role" {
  source  = "terraform-aws-modules/iam/aws//modules/iam-role-for-service-accounts-eks"
  version = "~> 5.0"

  role_name                     = "${var.cluster_name}-cert-manager-role"
  attach_cert_manager_policy    = true
  cert_manager_hosted_zone_arns = ["arn:aws:route53:::hostedzone/*"]

  oidc_providers = {
    main = {
      provider_arn = var.oidc_provider_arn
      namespace_service_accounts = [
        "cert-manager:cert-manager",
        "cert-manager:cert-manager-cainjector",
        "cert-manager:cert-manager-webhook"
      ]
    }
  }
}

# --- 6. K6 / STRESS TESTS ---
module "k6_irsa_role" {
  source  = "terraform-aws-modules/iam/aws//modules/iam-role-for-service-accounts-eks"
  version = "~> 5.0"

  role_name = "${var.cluster_name}-k6-role"

  oidc_providers = {
    main = {
      provider_arn               = var.oidc_provider_arn
      namespace_service_accounts = ["k6-operator-system:k6-operator"]
    }
  }
}


data "aws_acm_certificate" "issued" {
  domain      = "core-platform.click"
  statuses    = ["ISSUED"]
  most_recent = true
}



# --- 7. AWS SERVICE-LINKED ROLE FOR SPOT ---
# Ce rôle est obligatoire pour que Karpenter (ou tout service EC2) 
# puisse demander des instances Spot sur ce compte AWS.
resource "aws_iam_service_linked_role" "spot" {
  aws_service_name = "spot.amazonaws.com"

  # On ajoute un cycle de vie pour éviter les erreurs si le rôle existe déjà 
  # sur le compte AWS (car ce rôle est global au compte).
  lifecycle {
    ignore_changes = all
  }
}
# --- 7. EXTERNAL SECRETS OPERATOR (staging/prod only) ---
# Lets the ESO controller read the managed-backend credentials (MSK SCRAM, Redis
# AUTH) that the data-store modules write to Secrets Manager, and sync them into
# the app namespace as k8s Secrets.
module "external_secrets_irsa_role" {
  count = var.enable_external_secrets ? 1 : 0

  source  = "terraform-aws-modules/iam/aws//modules/iam-role-for-service-accounts-eks"
  version = "~> 5.0"

  role_name = "${var.cluster_name}-external-secrets-role"

  oidc_providers = {
    main = {
      provider_arn               = var.oidc_provider_arn
      namespace_service_accounts = ["external-secrets:external-secrets"]
    }
  }
}

resource "aws_iam_role_policy" "external_secrets_read" {
  count = var.enable_external_secrets ? 1 : 0

  name = "ExternalSecretsManagerRead"
  role = module.external_secrets_irsa_role[0].iam_role_name

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Sid    = "ReadProjectSecrets"
        Effect = "Allow"
        Action = ["secretsmanager:GetSecretValue", "secretsmanager:DescribeSecret"]
        Resource = [
          "arn:aws:secretsmanager:*:*:secret:AmazonMSK_${var.cluster_name}_*",
          "arn:aws:secretsmanager:*:*:secret:${var.cluster_name}-*"
        ]
      },
      {
        # The MSK SCRAM secret is encrypted with a customer KMS key.
        Sid      = "DecryptSecretCMKs"
        Effect   = "Allow"
        Action   = ["kms:Decrypt"]
        Resource = "*"
        Condition = {
          StringLike = { "kms:ViaService" = "secretsmanager.*.amazonaws.com" }
        }
      }
    ]
  })
}

# --- 8. AUDIT APP (KMS KEK + WORM bucket) ------------------------------------
# audit-server (sync RecordPrivileged seals PII; Query decrypts) and audit-worker
# (ingest seals; checkpoint anchors to WORM) are the SOLE principals on the audit
# KEK and the Object-Lock bucket. Write-only on the bucket: PutObject + retention,
# explicitly NO DeleteObject — WORM is enforced by Object-Lock COMPLIANCE, this
# just removes the ability from the role itself. Created only when the KEK ARN is
# supplied (staging/prod).
module "audit_app_irsa_role" {
  count = var.audit_kek_arn != "" ? 1 : 0

  source  = "terraform-aws-modules/iam/aws//modules/iam-role-for-service-accounts-eks"
  version = "~> 5.0"

  role_name = "${var.cluster_name}-audit-role"

  oidc_providers = {
    main = {
      provider_arn               = var.oidc_provider_arn
      namespace_service_accounts = var.audit_service_accounts
    }
  }
}

resource "aws_iam_role_policy" "audit_app" {
  count = var.audit_kek_arn != "" ? 1 : 0

  name = "AuditKekAndWorm"
  role = module.audit_app_irsa_role[0].iam_role_name

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Sid      = "WrapUnwrapSubjectDeks"
        Effect   = "Allow"
        Action   = ["kms:Decrypt", "kms:GenerateDataKey"]
        Resource = var.audit_kek_arn
      },
      {
        # Write + tamper-evident retention, NEVER delete/overwrite.
        Sid    = "WormWriteOnly"
        Effect = "Allow"
        Action = [
          "s3:PutObject",
          "s3:PutObjectRetention",
          "s3:GetObject",
          "s3:GetObjectRetention",
          "s3:ListBucket",
          "s3:GetBucketObjectLockConfiguration"
        ]
        Resource = [var.audit_worm_bucket_arn, "${var.audit_worm_bucket_arn}/*"]
      }
    ]
  })
}

# --- 9. MEDIA APP (asset bucket) ---------------------------------------------
# media-server brokers presigned upload/download + lifecycle, so it needs full
# object RW (incl. delete for asset takedown) on its bucket only. Created only
# when the media bucket ARN is supplied (staging/prod).
module "media_app_irsa_role" {
  count = var.media_bucket_arn != "" ? 1 : 0

  source  = "terraform-aws-modules/iam/aws//modules/iam-role-for-service-accounts-eks"
  version = "~> 5.0"

  role_name = "${var.cluster_name}-media-role"

  oidc_providers = {
    main = {
      provider_arn               = var.oidc_provider_arn
      namespace_service_accounts = var.media_service_accounts
    }
  }
}

resource "aws_iam_role_policy" "media_app" {
  count = var.media_bucket_arn != "" ? 1 : 0

  name = "MediaBucketReadWrite"
  role = module.media_app_irsa_role[0].iam_role_name

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Sid      = "ObjectReadWrite"
        Effect   = "Allow"
        Action   = ["s3:GetObject", "s3:PutObject", "s3:DeleteObject"]
        Resource = ["${var.media_bucket_arn}/*"]
      },
      {
        Sid      = "ListAndLocate"
        Effect   = "Allow"
        Action   = ["s3:ListBucket", "s3:GetBucketLocation"]
        Resource = [var.media_bucket_arn]
      }
    ]
  })
}

# --- 10. KARPENTER SPOT-INTERRUPTION QUEUE -----------------------------------
# Without this, Karpenter cannot receive the 2-minute spot interruption /
# rebalance / scheduled-maintenance notices, so it can't cordon+drain a reclaimed
# node gracefully — pods (incl. the realtime gateway's live WebSocket
# connections) are killed abruptly. EventBridge routes those events to this SQS
# queue; Karpenter polls it and starts a graceful drain on the warning. The queue
# NAME == cluster name (Karpenter convention; the platform appset sets
# settings.interruptionQueue from .global.clusterName).
resource "aws_sqs_queue" "karpenter_interruption" {
  name                      = var.cluster_name
  message_retention_seconds = 300
  sqs_managed_sse_enabled   = true
  tags                      = var.tags
}

data "aws_iam_policy_document" "karpenter_interruption_queue" {
  statement {
    sid       = "EventBridgeAndSpotToQueue"
    actions   = ["sqs:SendMessage"]
    resources = [aws_sqs_queue.karpenter_interruption.arn]
    principals {
      type        = "Service"
      identifiers = ["events.amazonaws.com", "sqs.amazonaws.com"]
    }
  }
}

resource "aws_sqs_queue_policy" "karpenter_interruption" {
  queue_url = aws_sqs_queue.karpenter_interruption.id
  policy    = data.aws_iam_policy_document.karpenter_interruption_queue.json
}

# The four event sources Karpenter consumes, each routed to the queue.
locals {
  karpenter_interruption_events = {
    spot_interruption = {
      source      = ["aws.ec2"]
      detail_type = ["EC2 Spot Instance Interruption Warning"]
    }
    rebalance = {
      source      = ["aws.ec2"]
      detail_type = ["EC2 Instance Rebalance Recommendation"]
    }
    instance_state_change = {
      source      = ["aws.ec2"]
      detail_type = ["EC2 Instance State-change Notification"]
    }
    scheduled_change = {
      source      = ["aws.health"]
      detail_type = ["AWS Health Event"]
    }
  }
}

resource "aws_cloudwatch_event_rule" "karpenter_interruption" {
  for_each = local.karpenter_interruption_events

  name          = "${var.cluster_name}-karpenter-${each.key}"
  event_pattern = jsonencode({ "source" = each.value.source, "detail-type" = each.value.detail_type })
  tags          = var.tags
}

resource "aws_cloudwatch_event_target" "karpenter_interruption" {
  for_each = local.karpenter_interruption_events

  rule = aws_cloudwatch_event_rule.karpenter_interruption[each.key].name
  arn  = aws_sqs_queue.karpenter_interruption.arn
}

# Let the Karpenter controller drain the interruption queue.
resource "aws_iam_role_policy" "karpenter_interruption" {
  name = "KarpenterInterruptionQueue"
  role = module.karpenter_irsa_role.iam_role_name

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Effect   = "Allow"
      Action   = ["sqs:DeleteMessage", "sqs:GetQueueUrl", "sqs:GetQueueAttributes", "sqs:ReceiveMessage"]
      Resource = aws_sqs_queue.karpenter_interruption.arn
    }]
  })
}
