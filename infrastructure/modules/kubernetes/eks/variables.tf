# infrastructure/modules/kubernetes/eks/variables.tf

# --- INFORMATIONS GÉNÉRALES ---

variable "project_name" {
  description = "Nom global du projet, utilisé pour le taggage des ressources AWS"
  type        = string
}

variable "env" {
  description = "Environnement de déploiement (ex: dev, staging, prod)"
  type        = string
}

variable "cluster_name" {
  description = "Nom unique du cluster EKS"
  type        = string
}

# --- RÉSEAU ---

variable "vpc_id" {
  description = "ID du VPC AWS où le cluster et ses nœuds seront déployés"
  type        = string
}

variable "private_subnet_ids" {
  description = "Liste des IDs de sous-réseaux privés pour l'hébergement des nœuds (sécurité accrue)"
  type        = list(string)
}

# --- CONFIGURATION DES NOEUDS SYSTEM ---

variable "system_node_settings" {
  description = "Configuration du groupe de nœuds critiques (CoreDNS, VPC CNI, etc.). Nécessite une haute disponibilité (min 2)."
  type = object({
    instance_types = list(string)
    min_size       = number
    max_size       = number
    desired_size   = number
  })
  default = {
    instance_types = ["t3.medium"]
    min_size       = 2
    max_size       = 3
    desired_size   = 2
  }
}

variable "node_groups" {
  description = "Map complète des Managed Node Groups (passée par Terragrunt)"
  type        = any
}

# --- SÉCURITÉ & IAM ---

variable "iam_policy_json_content" {
  description = "Document JSON définissant les permissions pour l'AWS Load Balancer Controller"
  type        = string
}