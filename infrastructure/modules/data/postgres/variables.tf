# infrastructure/modules/data/postgres/variables.tf

variable "project_name" { type = string }
variable "env"          { type = string }
variable "vpc_id"       { type = string }
variable "vpc_cidr"     { type = string }
variable "private_subnet_ids" { type = list(string) }

variable "db_name"     { type = string }
variable "db_username" { type = string }
variable "db_password" { type = string }

variable "engine_version" {
  type    = string
  default = "16.3"
}

variable "instance_class" {
  type    = string
  default = "db.t3.micro" # Gratuit ou très peu cher
}

variable "allocated_storage" {
  type    = number
  default = 20
}

variable "max_allocated_storage" {
  type    = number
  default = 100
}