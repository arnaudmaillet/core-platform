# infrastructure/modules/kubernetes/eks/variables.tf

variable "project_name" { type = string }
variable "env"          { type = string }
variable "cluster_name" { type = string }
variable "vpc_id"       { type = string }
variable "private_subnet_ids" { type = list(string) }

variable "eks_instance_types_system" {
  description = "Types d'instances pour les services système (CoreDNS, Karpenter, etc.)"
  type        = list(string)
  default     = ["t3.medium"]
}

variable "eks_instance_types_database" {
  description = "Types d'instances pour Postgres et ScyllaDB (besoin de plus de RAM)"
  type        = list(string)
  default     = ["t3.large"]
}

variable "eks_desired_size" {
  type    = number
  default = 5
}

variable "eks_min_size" {
  type    = number
  default = 3
}

variable "eks_max_size" {
  type    = number
  default = 10
}

variable "iam_policy_json_content" {
  type    = string
  description = "Contenu du fichier JSON de la policy IAM"
}