# infrastructure/modules/kubernetes/addons/sealed-secret.tf

resource "helm_release" "sealed_secrets" {
  name       = "sealed-secrets"
  repository = "https://bitnami-labs.github.io/sealed-secrets"
  chart      = "sealed-secrets"
  namespace  = "kube-system"
  # On le met dans kube-system pour qu'il soit disponible pour tout le cluster
}