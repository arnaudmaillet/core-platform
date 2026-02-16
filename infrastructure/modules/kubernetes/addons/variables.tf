# infrastructure/modules/kubernetes/addons/variables.tf

# --- CONNEXION AU CLUSTER ---

variable "cluster_name" {
  description = "Nom du cluster EKS (utilisé par Helm pour s'identifier)"
  type        = string
}

variable "cluster_endpoint" {
  description = "Endpoint de l'API Kubernetes"
  type        = string
}

variable "cluster_ca_certificate" {
  description = "Certificat CA du cluster pour l'authentification des providers"
  type        = string
}

# --- RÉSEAU ---

variable "vpc_id" {
  description = "ID du VPC où le Load Balancer doit être déployé"
  type        = string
}

variable "route53_zone_id" {
  description = "The ID of the Route53 hosted zone"
  type        = string
}

# --- RÔLES IAM (IRSA) ---

variable "lb_controller_role_arn" {
  description = "ARN du rôle IAM pour AWS Load Balancer Controller"
  type        = string
}

variable "ebs_csi_role_arn" {
  description = "ARN du rôle IAM pour le driver EBS CSI"
  type        = string
}

variable "eks_oidc_provider_arn" {
  description = "ARN of the EKS OIDC provider"
  type        = string
}

# --- AUTOSCALING & NODES ---

variable "karpenter_node_role_name" {
  description = "Nom du rôle IAM des nœuds pour la configuration de Karpenter"
  type        = string
}

# --- CERTIFICATS & DNS ---

variable "ssl_certificate_arn" {
  description = "ARN du certificat SSL (ACM) pour l'Ingress"
  type        = string
}