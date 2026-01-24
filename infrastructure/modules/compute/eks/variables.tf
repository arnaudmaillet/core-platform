variable "project_name" { type = string }
variable "env"          { type = string }
variable "vpc_id"       { type = string }
variable "private_subnets" { type = list(string) }