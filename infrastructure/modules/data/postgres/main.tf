# infrastructure/modules/data/postgres/main.tf

# --- DB PARAMETER GROUP ---
resource "aws_db_parameter_group" "this" {
  name   = "${var.project_name}-${var.env}-pg16-params"
  family = "postgres16"

  # Optimisation pour Rust/sqlx et monitoring
  parameter {
    name  = "log_min_duration_statement"
    value = "500"
  }

  # Paramètre utile si tu utilises des extensions comme PostGIS
  parameter {
    name  = "shared_preload_libraries"
    value = "pg_stat_statements"
  }
}

# --- GENERATION MOT DE PASSE (si non fourni) ---
resource "random_password" "db_password" {
  count   = var.db_password == "" ? 1 : 0
  length  = 20
  special = false # Évite les problèmes de caractères spéciaux dans les URLs de connexion
}

locals {
  actual_password = var.db_password == "" ? random_password.db_password[0].result : var.db_password
}

# --- INSTANCE RDS ---
resource "aws_db_instance" "this" {
  identifier           = "${var.project_name}-${var.env}-db"
  engine               = "postgres"
  engine_version       = var.engine_version
  instance_class       = var.instance_class
  allocated_storage    = var.allocated_storage
  max_allocated_storage = var.max_allocated_storage
  storage_type         = "gp3"
  storage_encrypted    = true # Sécurité par défaut

  db_name              = var.db_name
  username             = var.db_username
  password             = local.actual_password

  db_subnet_group_name   = aws_db_subnet_group.this.name
  vpc_security_group_ids = [aws_security_group.db.id]
  parameter_group_name   = aws_db_parameter_group.this.name

  # Dev settings
  backup_retention_period = 7
  deletion_protection     = false
  skip_final_snapshot     = true
  publicly_accessible     = false
  apply_immediately       = true

  tags = {
    Name        = "${var.project_name}-postgres"
    Environment = var.env
  }
}

resource "aws_db_subnet_group" "this" {
  name       = "${var.project_name}-${var.env}-db-subnet-group"
  subnet_ids = var.private_subnet_ids

  tags = {
    Name = "${var.project_name}-db-subnet-group"
  }
}

# --- SECRETS MANAGER ---
resource "aws_secretsmanager_secret" "db_url" {
  name                    = "${var.project_name}/${var.env}/profile-service/db-url"
  recovery_window_in_days = 0
}

resource "aws_secretsmanager_secret_version" "db_url_value" {
  secret_id     = aws_secretsmanager_secret.db_url.id
  secret_string = "postgres://${var.db_username}:${local.actual_password}@${aws_db_instance.this.endpoint}/${var.db_name}"
}

# --- SECURITY GROUP ---
resource "aws_security_group" "db" {
  name        = "${var.project_name}-${var.env}-db-sg"
  description = "Allow inbound traffic from EKS Apps Subnets"
  vpc_id      = var.vpc_id

  ingress {
    from_port   = 5432
    to_port     = 5432
    protocol    = "tcp"
    # Ici, on autorise tout le VPC, mais en prod on restreindra aux subnets EKS uniquement
    cidr_blocks = [var.vpc_cidr]
  }

  egress {
    from_port   = 0
    to_port     = 0
    protocol    = "-1"
    cidr_blocks = ["0.0.0.0/0"]
  }
}