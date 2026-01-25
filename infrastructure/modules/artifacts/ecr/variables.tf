variable "project_name" {
  type        = string
  description = "Nom du projet pour le nommage du repo"
}

variable "env" {
  type        = string
  description = "Environnement (dev, staging, prod)"
}