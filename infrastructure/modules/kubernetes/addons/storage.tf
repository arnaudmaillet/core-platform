# modules/addons/storage.tf

# 1. INSTALLATION DU DRIVER (Le moteur)
resource "helm_release" "aws_ebs_csi_driver" {
  name       = "aws-ebs-csi-driver"
  repository = "https://kubernetes-sigs.github.io/aws-ebs-csi-driver"
  chart      = "aws-ebs-csi-driver"
  namespace  = "kube-system"

  values = [yamlencode({
    controller = {
      nodeSelector = { intent = "system" }
      tolerations = [{
        key = "CriticalAddonsOnly", operator = "Equal", value = "true", effect = "NoSchedule"
      }]
      serviceAccount = {
        annotations = { "eks.amazonaws.com/role-arn" = var.ebs_csi_role_arn }
      }
    }
  })]
}

resource "kubernetes_storage_class_v1" "gp3" {
  metadata {
    name = "gp3"
    annotations = { "storageclass.kubernetes.io/is-default-class" = "true" }
  }
  storage_provisioner    = "ebs.csi.aws.com"
  reclaim_policy         = "Delete"
  allow_volume_expansion = true
  parameters             = { type = "gp3", encrypted = "true" }
  depends_on             = [helm_release.aws_ebs_csi_driver]
}