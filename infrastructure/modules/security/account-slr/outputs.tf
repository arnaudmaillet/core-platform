# infrastructure/modules/security/account-slr/outputs.tf
output "ensured" {
  description = "Marker that the account-global SLRs have been reconciled."
  value       = terraform_data.ec2_spot_slr.id
}
