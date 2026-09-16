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
  description = "Liste des noms des services à créer en tant que dépôts ECR"
}

variable "retained_generations" {
  type        = number
  default     = 5
  description = <<-EOT
    How many fleet generations (one merge to develop = one `:<git-sha>` manifest
    + its two per-arch images) each repository keeps, newest first. Bounds ECR
    storage and the rollback window: a `git revert` of a staging pin only works
    while the previous sha is still within this window. Images carrying a
    protected tag (see `protected_tag_prefixes`) are never counted out.
  EOT

  validation {
    condition     = var.retained_generations >= 1 && floor(var.retained_generations) == var.retained_generations
    error_message = "retained_generations must be a positive whole number."
  }
}

variable "untagged_expiry_days" {
  type        = number
  default     = 1
  description = "Days after which an untagged image (a superseded floating tag, an orphaned attestation/child manifest) is expired."

  validation {
    condition     = var.untagged_expiry_days >= 1
    error_message = "untagged_expiry_days must be at least 1."
  }
}

variable "image_architectures" {
  type        = list(string)
  default     = ["amd64", "arm64"]
  description = "Architecture suffixes of the per-arch images the fleet CI pushes (`:<git-sha>-<arch>`); one retention rule is generated per architecture."
}

variable "protected_tag_prefixes" {
  type        = list(string)
  default     = ["staging", "prod"]
  description = "Tag prefixes an environment is pinned/promoted to; an image carrying one is never expired, whatever its age."
}