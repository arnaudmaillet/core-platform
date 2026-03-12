# infrastructure/modules/kubernetes/argocd/bootstrap/main.tf

resource "local_file" "root_app_yaml" {
  content = <<EOF
apiVersion: argoproj.io/v1alpha1
kind: Application
metadata:
  name: root-bootstrap
  namespace: argocd
spec:
  project: default
  source:
    repoURL: ${var.repository_url}
    targetRevision: ${var.target_revision}
    path: infrastructure/argocd/bootstrap
  destination:
    server: https://kubernetes.default.svc
    namespace: argocd
  syncPolicy:
    automated:
      prune: true
      selfHeal: true
    syncOptions:
      - CreateNamespace=true
EOF
  filename = "${path.module}/root-app.yaml"
}

resource "null_resource" "apply_root_app" {
  depends_on = [local_file.root_app_yaml]

  provisioner "local-exec" {
    command = "sleep 10 && kubectl apply -f ${local_file.root_app_yaml.filename} --validate=false"
  }

  provisioner "local-exec" {
    when    = destroy
    command = "kubectl delete application root-bootstrap -n argocd --ignore-not-found"
  }
}