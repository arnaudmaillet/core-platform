# infrastructure/modules/kubernetes/eks/variables.tf

variable "project_name" { type = string }
variable "env"          { type = string }
variable "cluster_name" { type = string }
variable "vpc_id"       { type = string }
variable "private_subnet_ids" { type = list(string) }