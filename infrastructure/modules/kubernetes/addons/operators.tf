# infrastructure/modules/kubernetes/addons/databases.tf

# --- OPERATORS DE BASES DE DONNÉES ---

# CloudNative-PG : Pour gérer tes clusters PostgreSQL
resource "helm_release" "cnpg" {
  name             = "cloudnative-pg"
  repository       = "https://cloudnative-pg.github.io/charts"
  chart            = "cloudnative-pg"
  namespace        = "cnpg-system"
  create_namespace = true

  # On s'assure que le stockage par défaut est prêt
  depends_on = [kubernetes_storage_class_v1.gp3]
}

# ScyllaDB Operator : Pour la base NoSQL haute performance
resource "helm_release" "scylla_operator" {
  name             = "scylla-operator"
  repository       = "https://scylla-operator-charts.storage.googleapis.com/stable"
  chart            = "scylla-operator"
  namespace        = "scylla-operator"
  create_namespace = true

  # Scylla a besoin de cert-manager pour ses webhooks de validation
  depends_on = [
    helm_release.cert_manager,
    kubernetes_storage_class_v1.gp3
  ]
}