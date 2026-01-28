# infrastructure/modules/services/microservices/workers/outputs.tf

output "service_account_name" { value = kubernetes_service_account.this.metadata[0].name }
output "iam_role_arn"         { value = module.iam_eks_role.iam_role_arn }