# infrastructure/modules/kubernetes/argocd/addons/variables.tf

variable "cluster_name" {
  description = "Nom du cluster EKS"
  type        = string
}

variable "addons" {
  description = "Map des addons à installer"
  type = map(object({
    addon_version            = optional(string)
    service_account_role_arn = optional(string)
  }))
}

variable "tags" {
  description = "Tags à appliquer aux ressources"
  type        = map(string)
  default     = {}
}