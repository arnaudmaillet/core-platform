# infrastructure/modules/kubernetes/argocd-addons/outputs.tf

output "addon_arns" {
  value = { for k, v in aws_eks_addon.this : k => v.arn }
}