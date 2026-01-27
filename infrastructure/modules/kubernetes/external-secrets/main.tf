# infrastructure/modules/kubernetes/external-secret/main.tf

module "external_secrets_irsa" {
  source  = "terraform-aws-modules/iam/aws//modules/iam-role-for-service-accounts-eks"
  version = "~> 5.0"

  role_name = "${var.project_name}-external-secrets-irsa"

  oidc_providers = {
    main = {
      provider_arn               = var.oidc_provider_arn
      namespace_service_accounts = ["external-secrets:external-secrets"]
    }
  }

  # Permission large pour l'opérateur (il doit pouvoir lire les secrets de tous les services)
  role_policy_arns = {
    policy = "arn:aws:iam::aws:policy/SecretsManagerReadWrite" # Tu peux restreindre à ReadOnly
  }
}

# 2. Installer l'opérateur via Helm
resource "helm_release" "external_secrets" {
  name       = "external-secrets"
  repository = "https://charts.external-secrets.io"
  chart      = "external-secrets"
  namespace  = "external-secrets"
  create_namespace = true

  # On remplace les blocs "set" par un bloc "values" en YAML
  values = [
    yamlencode({
      installCRDs = true
      serviceAccount = {
        annotations = {
          "eks.amazonaws.com/role-arn" = module.external_secrets_irsa.iam_role_arn
        }
      }
    })
  ]
}