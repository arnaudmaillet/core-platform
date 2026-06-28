# infrastructure/modules/opensearch/main.tf
#
# Managed OpenSearch domain backing the search service's canonical inverted index
# (the SoReference read-model). VPC-attached, TLS, encryption at rest + node-to-
# node, with fine-grained access control via an internal master user.
#
# The master credentials are generated here and stored in Secrets Manager; the
# search pods read them via External Secrets (same pattern as MSK SCRAM / Redis
# AUTH). Index lifecycle (templates, blue-green reindex) is owned by the search
# service's Reindexer, NOT Terraform — this module only provisions the domain.

# ── Client security group ──────────────────────────────────────────────────────
resource "aws_security_group" "opensearch" {
  name_prefix = "${var.name}-opensearch-"
  description = "OpenSearch client access (HTTPS)"
  vpc_id      = var.vpc_id

  ingress {
    description = "HTTPS"
    from_port   = 443
    to_port     = 443
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

# ── Master user credentials (fine-grained access control) ──────────────────────
resource "random_password" "master" {
  length  = 32
  special = true
  # OpenSearch master password policy: at least one of each class.
  override_special = "!#$%^&*()-_=+[]{}"
}

resource "aws_secretsmanager_secret" "master" {
  name = "${var.name}-opensearch-master"
  tags = var.tags
}

resource "aws_secretsmanager_secret_version" "master" {
  secret_id = aws_secretsmanager_secret.master.id
  secret_string = jsonencode({
    username = var.master_user
    password = random_password.master.result
  })
}

# ── Domain ─────────────────────────────────────────────────────────────────────
resource "aws_opensearch_domain" "this" {
  domain_name    = var.name
  engine_version = var.engine_version

  cluster_config {
    instance_type          = var.instance_type
    instance_count         = var.instance_count
    zone_awareness_enabled = var.instance_count > 1

    dynamic "zone_awareness_config" {
      for_each = var.instance_count > 1 ? [1] : []
      content {
        availability_zone_count = min(var.instance_count, length(var.subnet_ids))
      }
    }
  }

  vpc_options {
    subnet_ids         = slice(var.subnet_ids, 0, min(var.instance_count, length(var.subnet_ids)))
    security_group_ids = [aws_security_group.opensearch.id]
  }

  ebs_options {
    ebs_enabled = true
    volume_type = "gp3"
    volume_size = var.volume_size
  }

  encrypt_at_rest {
    enabled = true
  }

  node_to_node_encryption {
    enabled = true
  }

  domain_endpoint_options {
    enforce_https       = true
    tls_security_policy = "Policy-Min-TLS-1-2-2019-07"
  }

  advanced_security_options {
    enabled                        = true
    internal_user_database_enabled = true
    master_user_options {
      master_user_name     = var.master_user
      master_user_password = random_password.master.result
    }
  }

  tags = var.tags
}
