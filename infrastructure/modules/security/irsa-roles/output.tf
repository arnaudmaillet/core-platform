# infrastructure/modules/kubernetes/identity/output.tf

output "lb_controller_role_arn" {
  value = module.lb_controller_irsa_role.iam_role_arn
}

output "external_dns_role_arn" {
  value = module.external_dns_irsa_role.iam_role_arn
}

output "karpenter_role_arn" {
  value = module.karpenter_irsa_role.iam_role_arn
}

output "cert_manager_role_arn" {
  value = module.cert_manager_irsa_role.iam_role_arn
}

output "k6_role_arn" {
  value = module.k6_irsa_role.iam_role_arn
}

output "ebs_csi_role_arn" {
  value = module.ebs_csi_irsa_role.iam_role_arn
}
output "certificate_arn" {
  value = data.aws_acm_certificate.issued.arn
}

output "external_secrets_role_arn" {
  description = "IRSA role ARN for External Secrets Operator (null unless enable_external_secrets)."
  value       = try(module.external_secrets_irsa_role[0].iam_role_arn, null)
}

# La map globale que ton module ArgoCD va adorer consommer
output "iam_role_arns" {
  value = {
    lb_controller   = module.lb_controller_irsa_role.iam_role_arn
    external_dns    = module.external_dns_irsa_role.iam_role_arn
    karpenter       = module.karpenter_irsa_role.iam_role_arn
    cert_manager    = module.cert_manager_irsa_role.iam_role_arn
    k6              = module.k6_irsa_role.iam_role_arn
    ebs_csi         = module.ebs_csi_irsa_role.iam_role_arn
    external_secrets = try(module.external_secrets_irsa_role[0].iam_role_arn, null)
  }
}