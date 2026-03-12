# infrastructure/modules/kubernetes/argocd/bootstrap/variables.tf

variable "repository_url" {
  description = "URL du dépôt Git contenant l'infrastructure"
  type        = string
}

variable "target_revision" {
  description = "Branche, tag ou commit à synchroniser (ex: env/dev)"
  type        = string
  default     = "HEAD"
}