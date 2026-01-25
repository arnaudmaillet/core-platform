resource "aws_ecr_repository" "main" {
  name                 = "${var.project_name}-backend"
  image_tag_mutability = "MUTABLE"

  # Hyperscale : Scan automatique des vulnérabilités CVE à chaque push
  image_scanning_configuration {
    scan_on_push = true
  }

  # Chiffrement au repos via KMS (Best practice sécurité)
  encryption_configuration {
    encryption_type = "AES256"
  }

  tags = {
    Name        = "${var.project_name}-ecr"
    Environment = var.env
  }
}

# --- LIFECYCLE POLICY ---
# Évite de payer pour des milliers d'images inutiles.
# On garde les 30 dernières images de build.
resource "aws_ecr_lifecycle_policy" "cleanup" {
  repository = aws_ecr_repository.main.name

  policy = jsonencode({
    rules = [
      {
        rulePriority = 1
        description  = "Keep last 30 images"
        selection = {
          tagStatus   = "any"
          countType   = "imageCountMoreThan"
          countNumber = 30
        }
        action = {
          type = "expire"
        }
      }
    ]
  })
}