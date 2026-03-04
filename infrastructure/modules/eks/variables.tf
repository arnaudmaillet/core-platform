# infrastructure/modules/eks/variables.tf

# --- INFORMATIONS GÉNÉRALES ---
variable "project_name" {
  description = "Nom global du projet"
  type        = string
}

variable "env" {
  description = "Environnement (dev, staging, prod)"
  type        = string
}

variable "cluster_name" {
  description = "Nom unique du cluster EKS"
  type        = string
}

# --- RÉSEAU ---
variable "vpc_id" {
  description = "ID du VPC AWS"
  type        = string
}

variable "private_subnet_ids" {
  description = "Liste des IDs de sous-réseaux privés"
  type        = list(string)
}

# --- CONFIGURATION DES NOEUDS ---
variable "node_groups" {
  description = "Map complète des Managed Node Groups (passée par Terragrunt)"
  type        = any
}