# infrastructure/live/global/security/ec2-spot-slr/terragrunt.hcl
#
# Ensures the account-global EC2 Spot service-linked role exists. Applied ONCE
# for the account (global tree), independent of any env's lifecycle — so an env
# teardown never deletes it and never breaks Karpenter spot in the other envs.
# Formerly created per-env inside modules/security/irsa-roles (removed there).

include "root" {
  path = find_in_parent_folders("root.hcl")
}

terraform {
  source = "../../../../modules/security/account-slr"
}

inputs = {
  aws_region = "us-east-1"
}
