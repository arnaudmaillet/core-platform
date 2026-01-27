# infrastructure/modules/data/postgres/main.tf

resource "aws_db_instance" "this" {
  identifier           = "${var.project_name}-${var.env}-db"
  engine               = "postgres"
  engine_version       = var.engine_version
  instance_class       = var.instance_class
  allocated_storage    = var.allocated_storage
  max_allocated_storage = var.max_allocated_storage # Autoscale le disque si besoin
  storage_type         = "gp3"

  db_name              = var.db_name
  username             = var.db_username
  password             = var.db_password

  db_subnet_group_name   = aws_db_subnet_group.this.name
  vpc_security_group_ids = [aws_security_group.db.id]

  # Configuration pour le développement
  backup_retention_period = 7
  deletion_protection     = false
  skip_final_snapshot     = true
  publicly_accessible     = false

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

resource "aws_secretsmanager_secret" "db_url" {
  name        = "${var.project_name}/${var.env}/profile-service/db-url"
  recovery_window_in_days = 0 # Permet de supprimer/recréer sans attendre 30 jours (pratique en dev)
}

resource "aws_secretsmanager_secret_version" "db_url_value" {
  secret_id     = aws_secretsmanager_secret.db_url.id
  secret_string = "postgres://${var.db_username}:${var.db_password}@${aws_db_instance.this.endpoint}/${var.db_name}"
}

resource "aws_security_group" "db" {
  name        = "${var.project_name}-${var.env}-db-sg"
  description = "Allow inbound traffic from EKS"
  vpc_id      = var.vpc_id

  # Autorise le trafic PostgreSQL (5432) venant du VPC (EKS)
  ingress {
    from_port   = 5432
    to_port     = 5432
    protocol    = "tcp"
    cidr_blocks = [var.vpc_cidr]
  }

  egress {
    from_port   = 0
    to_port     = 0
    protocol    = "-1"
    cidr_blocks = ["0.0.0.0/0"]
  }
}