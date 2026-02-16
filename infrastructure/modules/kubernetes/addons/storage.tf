# modules/addons/storage.tf

# 1. INSTALLATION DU DRIVER (Le moteur)
# Permet à EKS de communiquer avec les API AWS EBS pour créer des disques
resource "helm_release" "aws_ebs_csi_driver" {
  name       = "aws-ebs-csi-driver"
  repository = "https://kubernetes-sigs.github.io/aws-ebs-csi-driver"
  chart      = "aws-ebs-csi-driver"
  namespace  = "kube-system"

  set = [
    {
      name  = "controller.serviceAccount.create"
      value = "true"
    },
    {
      name  = "controller.serviceAccount.name"
      value = "ebs-csi-controller-sa"
    },
    {
      name  = "controller.serviceAccount.annotations.eks\\.amazonaws\\.com/role-arn"
      value = var.ebs_csi_role_arn
    }
  ]
}

# 2. CONFIGURATION DU TYPE DE DISQUE (La règle)
# Définit comment les volumes doivent être provisionnés
resource "kubernetes_storage_class_v1" "gp3" {
  metadata {
    name = "gp3"
    annotations = {
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

  # On s'assure que la StorageClass n'est créée qu'une fois le driver prêt
  depends_on = [helm_release.aws_ebs_csi_driver]
}