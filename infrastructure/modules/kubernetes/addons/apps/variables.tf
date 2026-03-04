# infrastructure/modules/kubernetes/addons/apps/variables.tf

# --- CONNEXION AU CLUSTER (Commun aux deux) ---
variable "cluster_name"           { type = string }
variable "aws_region"             { type = string }
variable "cluster_endpoint"       { type = string }
variable "cluster_ca_certificate" { type = string }

# --- CERTIFICATS & DNS (APPS UNIQUEMENT) ---
variable "route53_zone_id"     { type = string }
variable "ssl_certificate_arn" { type = string }
variable "external_dns_role_arn" { type = string }

# --- RÔLES IAM (CORE UNIQUEMENT) ---
variable "karpenter_controller_role_arn" { type = string }