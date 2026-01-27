# infrastructure/modules/artifacts/ecr/variables.tf

variable "project_name" {
  type        = string
  description = "Nom du projet (ex: core-platform)"
}

variable "env" {
  type        = string
  description = "Environnement (dev, prod, etc.)"
}

variable "service_names" {
  type        = list(string)
  description = "Liste des noms des microservices à créer"
  default     = ["graphql-bff", "profile-service"]
}