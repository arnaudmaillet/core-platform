# infrastructure/modules/kubernetes/storage/output.tf

output "storage_class_name" {
  description = "Le nom de la StorageClass créée"
  value       = kubernetes_storage_class_v1.gp3.metadata[0].name
}

output "helm_release_status" {
  description = "Statut du déploiement Helm du driver"
  value       = helm_release.aws_ebs_csi_driver.status
}