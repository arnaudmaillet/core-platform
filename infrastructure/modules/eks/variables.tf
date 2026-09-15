# infrastructure/modules/eks/variables.tf

# --- INFORMATIONS GÉNÉRALES ---
variable "project_name" {
  description = "Nom global du projet"
  type        = string
}

variable "env" {
  description = "Environnement (dev, staging, prod)"
  type        = string
}

variable "cluster_name" {
  description = "Nom unique du cluster EKS"
  type        = string
}

# --- RÉSEAU ---
variable "vpc_id" {
  description = "ID du VPC AWS"
  type        = string
}

variable "private_subnet_ids" {
  description = "Liste des IDs de sous-réseaux privés"
  type        = list(string)
}

variable "vpc_cidr" {
  description = "VPC CIDR — used to scope the node SG rule that lets the ALB reach ArgoCD (8080)."
  type        = string
}

variable "endpoint_public_access_cidrs" {
  description = "CIDRs allowed to reach the EKS public API endpoint. Default 0.0.0.0/0 — TIGHTEN per-env to your admin/CI ranges."
  type        = list(string)
  default     = ["0.0.0.0/0"]
}

variable "cluster_version" {
  description = <<-EOT
    Kubernetes minor for the control plane. Keep it on a version in EKS STANDARD
    support (`aws eks describe-cluster-versions`): a version in extended support
    is billed 0.60 USD/h instead of 0.10 (6x), and at the end of extended support
    AWS force-upgrades the control plane at an unannounced time. Bump this before
    the end-of-standard-support date of the pinned version (1.36: 2027-08-02).
    Companion pins that gate a bump: Karpenter (min version per k8s minor —
    karpenter.sh/docs/upgrading/compatibility), KEDA, CNPG, ESO, cert-manager.
  EOT
  type        = string
  default     = "1.36"
}

variable "cluster_support_type" {
  description = "EKS upgrade policy support_type: STANDARD (auto-upgrade at end of standard support, never billed extended support) or EXTENDED (AWS default; may sit in extended support at 6x the control-plane price)."
  type        = string
  default     = "EXTENDED"
  validation {
    condition     = contains(["STANDARD", "EXTENDED"], var.cluster_support_type)
    error_message = "cluster_support_type must be STANDARD or EXTENDED."
  }
}

# --- CONFIGURATION DES NOEUDS ---
variable "node_groups" {
  description = "Map complète des Managed Node Groups (passée par Terragrunt)"
  type        = any
}