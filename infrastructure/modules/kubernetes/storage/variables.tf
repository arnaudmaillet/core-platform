# infrastructure/modules/kubernetes/storage/variables.tf

variable "ebs_csi_role_arn" {
  description = "ARN du rôle IAM IRSA pour le driver EBS CSI"
  type        = string
}

variable "cluster_name" {
  description = "Nom du cluster EKS"
  type        = string
  default     = ""
}

variable "cluster_endpoint" {
  description = "Endpoint de l'API Kubernetes"
  type        = string
}

variable "cluster_ca_certificate" {
  description = "Certificat CA du cluster"
  type        = string
}
