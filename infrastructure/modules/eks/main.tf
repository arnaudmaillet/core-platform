# infrastructure/modules/eks/main.tf

# --- MODULE EKS PRINCIPAL ---
module "eks" {
  source  = "terraform-aws-modules/eks/aws"
  version = "~> 20.0"

  cluster_name    = var.cluster_name
  cluster_version = "1.31"

  # Private access is always on (in-VPC/node traffic stays off the public path).
  # Public access stays on so kubectl/Terraform reach the API, but the allow-list
  # is now a knob (W3): default 0.0.0.0/0 — tighten per-env to your admin/CI CIDRs.
  cluster_endpoint_public_access       = true
  cluster_endpoint_private_access      = true
  cluster_endpoint_public_access_cidrs = var.endpoint_public_access_cidrs

  vpc_id     = var.vpc_id
  subnet_ids = var.private_subnet_ids

  enable_cluster_creator_admin_permissions = true
  enable_irsa                              = true

  # Enable NetworkPolicy enforcement in the AWS VPC CNI — without this the CNI
  # silently ignores NetworkPolicy objects, so the fleet's namespace-isolation
  # baseline (k8s/overlays/staging/networkpolicies.yaml) would be a no-op. The CNI
  # network-policy agent permits the node->pod kubelet health-probe path, so
  # default-deny ingress does not break liveness/readiness probes.
  cluster_addons = {
    vpc-cni = {
      configuration_values = jsonencode({
        enableNetworkPolicy = "true"
      })
    }
  }

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
      # The ALB lives in the VPC; allow from the actual VPC CIDR (W4 — was
      # hardcoded to 10.0.0.0/16, wrong for any env not on that CIDR, e.g. staging
      # 10.20.0.0/16). TODO: tighten to the ALB security group once the LB
      # controller has created it.
      cidr_blocks = [var.vpc_cidr]
    }
  }
}