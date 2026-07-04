# infrastructure/modules/msk/main.tf
#
# Managed Kafka (MSK) for the fleet, exposing SASL/SCRAM over TLS — exactly the
# auth the storage/transport crates read from env (KAFKA_SECURITY_PROTOCOL=
# SASL_SSL, KAFKA_SASL_MECHANISM=SCRAM-SHA-512, KAFKA_SASL_USERNAME/PASSWORD).
#
# Thin wrapper over terraform-aws-modules/msk-kafka-cluster (repo convention).
# The generated SCRAM credential is stored in Secrets Manager (the MSK-required
# `AmazonMSK_` prefix + a customer KMS key) and associated with the cluster; the
# app side reads it via External Secrets Operator.

# ── Broker security group ──────────────────────────────────────────────────────
resource "aws_security_group" "broker" {
  name_prefix = "${var.name}-msk-"
  description = "MSK broker client access"
  vpc_id      = var.vpc_id

  ingress {
    description = "Kafka TLS"
    from_port   = 9094
    to_port     = 9094
    protocol    = "tcp"
    cidr_blocks = var.allowed_cidr_blocks
  }

  ingress {
    description = "Kafka SASL/SCRAM over TLS"
    from_port   = 9096
    to_port     = 9096
    protocol    = "tcp"
    cidr_blocks = var.allowed_cidr_blocks
  }

  egress {
    from_port   = 0
    to_port     = 0
    protocol    = "-1"
    cidr_blocks = ["0.0.0.0/0"]
  }

  tags = var.tags

  lifecycle {
    create_before_destroy = true
  }
}

# ── SCRAM credential (MSK requires a CMK-encrypted AmazonMSK_-prefixed secret) ──
resource "aws_kms_key" "scram" {
  description         = "${var.name} MSK SCRAM credential"
  enable_key_rotation = true
  tags                = var.tags
}

resource "random_password" "scram" {
  length  = 32
  special = false
}

resource "aws_secretsmanager_secret" "scram" {
  name       = "AmazonMSK_${var.name}_app"
  kms_key_id = aws_kms_key.scram.arn
  # Disposable envs set this to 0 so a destroy frees the name immediately;
  # otherwise the recovery window reserves it and the next apply collides with
  # "secret already scheduled for deletion". Default keeps the AWS-standard window.
  recovery_window_in_days = var.secret_recovery_window_days
  tags                    = var.tags
}

resource "aws_secretsmanager_secret_version" "scram" {
  secret_id     = aws_secretsmanager_secret.scram.id
  secret_string = jsonencode({ username = "app", password = random_password.scram.result })
}

# ── Cluster ────────────────────────────────────────────────────────────────────
module "msk" {
  source  = "terraform-aws-modules/msk-kafka-cluster/aws"
  version = "~> 2.0"

  name                   = var.name
  kafka_version          = var.kafka_version
  number_of_broker_nodes = var.number_of_broker_nodes

  broker_node_instance_type   = var.broker_instance_type
  broker_node_client_subnets  = var.subnet_ids
  broker_node_security_groups = [aws_security_group.broker.id]
  broker_node_storage_info = {
    ebs_storage_info = { volume_size = var.broker_ebs_volume_size }
  }

  # ── Server properties (explicit, not broker defaults) ────────────────────────
  # Topics are provisioned by the topic-provisioner PreSync Job from the
  # event-topology registry; auto-creation stays OFF (MSK's own default — made
  # explicit here as policy) so a typo'd topic name fails loudly instead of
  # spawning a phantom topic with defaults nobody chose. The replication settings
  # below apply to broker-side topic creation paths; the provisioner sets RF
  # explicitly per topic (TOPIC_REPLICATION_FACTOR).
  configuration_name        = "${var.name}-server-properties"
  configuration_description = "${var.name} fleet server properties"
  configuration_server_properties = {
    "auto.create.topics.enable"  = "false"
    "default.replication.factor" = tostring(var.default_replication_factor)
    "min.insync.replicas"        = tostring(var.min_insync_replicas)
    "log.retention.hours"        = tostring(var.log_retention_hours)
  }

  encryption_in_transit_client_broker = "TLS"

  client_authentication = {
    sasl = { scram = true }
  }

  # Associate the generated SCRAM secret with the cluster.
  create_scram_secret_association          = true
  scram_secret_association_secret_arn_list = [aws_secretsmanager_secret.scram.arn]

  tags = var.tags
}
