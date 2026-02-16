# infrastructure/modules/kubernetes/addons/cert-manager.tf

# --- 1. CERT-MANAGER (Pré-requis pour Scylla/CNPG) ---
resource "helm_release" "cert_manager" {
  name             = "cert-manager"
  repository       = "https://charts.jetstack.io"
  chart            = "cert-manager"
  version          = "v1.13.0"
  namespace        = "cert-manager"
  create_namespace = true
  set  = [
    {
      name  = "installCRDs"
      value = "true"
    }
  ]
}