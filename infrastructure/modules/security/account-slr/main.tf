# infrastructure/modules/security/account-slr/main.tf
#
# Account-global AWS service-linked roles that must exist ONCE per account and
# must OUTLIVE any single environment's teardown. Managed in the global tree,
# never per-env: a per-env `aws_iam_service_linked_role` gets DELETED on that
# env's destroy, which (a) fails with "unexpected state" while spot capacity is
# still draining and (b) would break Karpenter spot in every OTHER env that
# shares the account (seen live 2026-07-04, staging teardown).
#
# Idempotent + destroy-safe by construction: `terraform_data` + local-exec runs
# `create-service-linked-role`, tolerating "already exists" (the role may have
# been created by an earlier env, a prior run, or auto-created by AWS on first
# spot launch). There is no destroy provisioner, so `terraform destroy` here is
# a no-op on the real role — exactly the intent for an account-global primitive.

variable "aws_region" {
  type    = string
  default = "us-east-1"
}

# EC2 Spot SLR — required for Karpenter (and any service) to request Spot.
resource "terraform_data" "ec2_spot_slr" {
  provisioner "local-exec" {
    command = <<-SH
      aws iam create-service-linked-role --aws-service-name spot.amazonaws.com \
        --region ${var.aws_region} 2>/dev/null \
        || echo "AWSServiceRoleForEC2Spot already exists (account-global); continuing."
    SH
  }
}
