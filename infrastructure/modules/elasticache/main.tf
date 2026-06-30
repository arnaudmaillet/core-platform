# infrastructure/modules/elasticache/main.tf
#
# Managed Redis (ElastiCache) in cluster mode, TLS + AUTH — matching the env the
# redis-storage crate reads (REDIS_TOPOLOGY=cluster, REDIS_HOSTS=<configuration
# endpoint>, REDIS_PASSWORD=<auth token>, TLS on).
#
# Thin wrapper over terraform-aws-modules/elasticache (repo convention). The AUTH
# token is generated and stored in Secrets Manager; the app reads it via External
# Secrets Operator. Staging consolidates dev's 7 per-service caches into one
# managed cluster (services already hash-tag their keys).

# ── Client security group ──────────────────────────────────────────────────────
resource "aws_security_group" "redis" {
  name_prefix = "${var.name}-redis-"
  description = "ElastiCache Redis client access"
  vpc_id      = var.vpc_id

  ingress {
    description = "Redis"
    from_port   = 6379
    to_port     = 6379
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

# ── AUTH token (requires transit encryption; stored for the app via ESO) ───────
resource "random_password" "auth" {
  length  = 64
  special = false # ElastiCache AUTH tokens reject most special chars
}

resource "aws_secretsmanager_secret" "auth" {
  name = "${var.name}-redis-auth"
  # Disposable envs set 0 so a destroy frees the name immediately (otherwise the
  # recovery window reserves it and the next apply collides). Default = AWS window.
  recovery_window_in_days = var.secret_recovery_window_days
  tags                    = var.tags
}

resource "aws_secretsmanager_secret_version" "auth" {
  secret_id     = aws_secretsmanager_secret.auth.id
  secret_string = jsonencode({ password = random_password.auth.result })
}

# ── Replication group ──────────────────────────────────────────────────────────
module "redis" {
  source  = "terraform-aws-modules/elasticache/aws"
  version = "~> 1.0"

  replication_group_id = var.name
  description          = "${var.name} shared Redis (cluster mode)"

  engine         = "redis"
  engine_version = var.engine_version
  node_type      = var.node_type
  port           = 6379

  cluster_mode_enabled       = true
  num_node_groups            = var.num_node_groups
  replicas_per_node_group    = var.replicas_per_node_group
  automatic_failover_enabled = true
  multi_az_enabled           = true

  transit_encryption_enabled = true
  at_rest_encryption_enabled = true
  auth_token                 = random_password.auth.result

  # Networking: module-managed subnet group, our own SG. Disable the module's
  # own SG creation — it builds one without a vpc_id and falls back to the
  # (non-existent) default VPC, failing with VPCIdNotSpecified.
  create_security_group = false
  create_subnet_group   = true
  subnet_group_name     = var.name
  subnet_ids            = var.subnet_ids
  security_group_ids    = [aws_security_group.redis.id]

  tags = var.tags
}
