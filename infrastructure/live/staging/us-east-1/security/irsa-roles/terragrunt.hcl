# infrastructure/live/staging/us-east-1/security/irsa-roles/terragrunt.hcl
# Mirrors the dev IRSA component, plus the External Secrets Operator role
# (enable_external_secrets) so the staging app can read MSK/Redis creds from
# Secrets Manager.

include "root" {
  path = find_in_parent_folders("root.hcl")
}

dependency "eks" {
  config_path = "../../eks"

  # Destroy-only mocks: after a partial teardown the eks unit's outputs are gone
  # from state and this unit's destroy dies at HCL evaluation ("no variable named
  # dependency") — seen live on the 2026-07-04 staging teardown. Inputs are not
  # consulted when destroying from state, so mocks are safe here.
  mock_outputs_allowed_terraform_commands = ["destroy"]
  mock_outputs = {
    cluster_name       = "mock-cluster"
    oidc_provider_arn  = "arn:aws:iam::000000000000:oidc-provider/mock"
    oidc_provider_url  = "oidc.eks.us-east-1.amazonaws.com/id/MOCK"
    node_iam_role_arns = ["arn:aws:iam::000000000000:role/mock"]
  }
}

# App IRSA scopes its policies to the exact Block 3 data-store ARNs.
dependency "audit_kms" {
  config_path                             = "../../data/audit-kms"
  mock_outputs                            = { key_arn = "arn:aws:kms:us-east-1:000000000000:key/mock" }
  mock_outputs_allowed_terraform_commands = ["validate", "plan", "destroy"]
}

dependency "audit_worm" {
  config_path                             = "../../data/audit-worm"
  mock_outputs                            = { bucket_arn = "arn:aws:s3:::mock-audit-worm" }
  mock_outputs_allowed_terraform_commands = ["validate", "plan", "destroy"]
}

dependency "media_bucket" {
  config_path                             = "../../data/media-bucket"
  mock_outputs                            = { bucket_arn = "arn:aws:s3:::mock-media" }
  mock_outputs_allowed_terraform_commands = ["validate", "plan", "destroy"]
}

dependency "cnpg_backups" {
  config_path                             = "../../data/cnpg-backups"
  mock_outputs                            = { bucket_arn = "arn:aws:s3:::mock-cnpg-backups" }
  mock_outputs_allowed_terraform_commands = ["validate", "plan", "destroy"]
}

terraform {
  source = "../../../../../modules/security/irsa-roles"
}

inputs = {
  cluster_name       = dependency.eks.outputs.cluster_name
  oidc_provider_arn  = dependency.eks.outputs.oidc_provider_arn
  oidc_provider_url  = dependency.eks.outputs.oidc_provider_url
  node_iam_role_arns = dependency.eks.outputs.node_iam_role_arns

  iam_policy_json_content = file("${get_repo_root()}/infrastructure/assets/iam-policies/aws-lb-controller.json")

  # Staging consumes managed backends -> ESO reads their Secrets Manager creds.
  enable_external_secrets = true

  # ── Application IRSA (scoped to the Block 3 data stores) ──────────────────
  # SA subjects carry the overlay's `staging-` namePrefix, in namespace `default`.
  audit_kek_arn         = dependency.audit_kms.outputs.key_arn
  audit_worm_bucket_arn = dependency.audit_worm.outputs.bucket_arn
  audit_service_accounts = [
    "default:staging-audit-server",
    "default:staging-audit-worker",
  ]
  media_bucket_arn       = dependency.media_bucket.outputs.bucket_arn
  media_service_accounts = ["default:staging-media-server"]

  # CNPG backup role — each cluster's pod SA (name == cluster name) assumes it.
  cnpg_backup_bucket_arn = dependency.cnpg_backups.outputs.bucket_arn
  cnpg_service_accounts = [
    "default:staging-account-postgres",
    "default:staging-counter-postgres",
    "default:staging-audit-postgres",
    "default:staging-moderation-postgres",
    "default:staging-auth-postgres",
    "default:staging-media-postgres",
  ]

  tags = {
    Environment = "staging"
    ManagedBy   = "terragrunt"
  }
}
