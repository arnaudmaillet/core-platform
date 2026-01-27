# infrastructure/modules/networking/variables.tf

variable "project_name" {
  type        = string
  description = "Nom du projet"
}

variable "env" {
  type        = string
  description = "Environnement (dev, staging, prod)"
}

variable "cluster_name" {
  type        = string
  description = "Nom du cluster EKS pour le tagging"
}

variable "vpc_cidr" {
  type    = string
  default = "10.0.0.0/16"
}

variable "availability_zones" {
  type    = list(string)
  default = ["eu-west-3a", "eu-west-3b", "eu-west-3c"]
}