# infrastructure/modules/networking/vpc/variables.tf

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
  default = ["us-east-1a", "us-east-1b", "us-east-1c"]
}

variable "aws_region" {
  type        = string
  description = "Region for VPC-endpoint service names (e.g. us-east-1)."
  # Provided globally via root.hcl inputs; default keeps the module usable standalone.
  default = "us-east-1"
}

variable "single_nat_gateway" {
  type        = bool
  description = "true = one shared NAT (cheap); false = one NAT per AZ (HA, in-AZ egress)."
  default     = true
}

variable "enable_interface_endpoints" {
  type        = bool
  description = "Provision interface endpoints (ECR/STS/SecretsManager/KMS) to keep AWS-API calls off the NAT. Bills per-AZ-hour; enable in staging/prod."
  default     = false
}

variable "interface_endpoint_services" {
  type        = list(string)
  description = "Short service names for the interface endpoints (region prefix added in main.tf)."
  default     = ["ecr.api", "ecr.dkr", "sts", "secretsmanager", "kms"]
}