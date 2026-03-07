# infrastructure/modules/kubernetes/storage/main.tf

# Driver EBS CSI pour permettre à EKS de créer des volumes AWS EBS
resource "helm_release" "aws_ebs_csi_driver" {
  name       = "aws-ebs-csi-driver"
  repository = "https://kubernetes-sigs.github.io/aws-ebs-csi-driver"
  chart      = "aws-ebs-csi-driver"
  namespace  = "kube-system"
  version    = "2.27.0" # Version stable du chart

  values = [yamlencode({
    controller = {
      serviceAccount = {
        create = true
        name   = "ebs-csi-controller-sa"
        annotations = {
          "eks.amazonaws.com/role-arn" = var.ebs_csi_role_arn
        }
      }
      # On retire les sélecteurs trop restrictifs pour éviter que le driver reste en Pending
      nodeSelector = {
        "kubernetes.io/os" = "linux"
      }
    }
    node = {
      tolerateAllTaints = true
    }
  })]
}

# Définition de la StorageClass gp3 (plus performante et moins chère que gp2)
resource "kubernetes_storage_class_v1" "gp3" {
  metadata {
    name = "gp3"
    annotations = {
      # On définit gp3 comme la classe par défaut du cluster
      "storageclass.kubernetes.io/is-default-class" = "true"
    }
  }

  storage_provisioner    = "ebs.csi.aws.com"
  reclaim_policy         = "Delete"
  allow_volume_expansion = true
  volume_binding_mode    = "WaitForFirstConsumer"

  parameters = {
    type      = "gp3"
    encrypted = "true"
  }

  depends_on = [helm_release.aws_ebs_csi_driver]
}