# infrastructure/live/prod/us-east-1/data/msk/terragrunt.hcl

include "root" {
  path = find_in_parent_folders("root.hcl")
}

dependency "vpc" {
  config_path = "../../networking/vpc"
}

terraform {
  source = "../../../../../modules/msk"
}

locals {
  env_vars = read_terragrunt_config(find_in_parent_folders("env.hcl"))
}

inputs = {
  name       = "core-platform-${local.env_vars.locals.env}"
  vpc_id     = dependency.vpc.outputs.vpc_id
  subnet_ids = dependency.vpc.outputs.private_data_subnet_ids
  # 3 brokers across the 3 data-subnet AZs — the durable-Kafka floor: RF 3 with
  # min.insync.replicas 2 (module defaults) survives one broker/AZ down while
  # acks=all still commits.
  number_of_broker_nodes = 3
  broker_instance_type   = "kafka.m5.large" # t3.small burst credits are not a prod posture
  broker_ebs_volume_size = 100
  allowed_cidr_blocks    = [dependency.vpc.outputs.vpc_cidr_block]

  # RF 3 / min.insync 2 come from the module defaults. Keep the prod overlay's
  # topic-provisioner Job TOPIC_REPLICATION_FACTOR=3 in lockstep.

  tags = {
    Environment = local.env_vars.locals.env
    ManagedBy   = "terragrunt"
  }
}
