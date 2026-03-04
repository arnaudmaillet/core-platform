# infrastructure/modules/kubernetes/addons/core/variables.tf

# --- CONNEXION AU CLUSTER (Commun aux deux) ---
variable "cluster_name"           { type = string }
variable "aws_region"             { type = string }
variable "cluster_endpoint"       { type = string }
variable "cluster_ca_certificate" { type = string }

# --- RÉSEAU ---
variable "vpc_id" {
  description = "ID du VPC pour le Load Balancer"
  type        = string
}

# --- RÔLES IAM (CORE UNIQUEMENT) ---
variable "lb_controller_role_arn"        { type = string }
variable "ebs_csi_role_arn"              { type = string }
variable "eks_oidc_provider_arn"         { type = string }
variable "karpenter_controller_role_arn" { type = string }
variable "karpenter_node_role_name"      { type = string }