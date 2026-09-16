# infrastructure/modules/artifacts/ecr/main.tf

resource "aws_ecr_repository" "services" {
  # On boucle sur la liste des services pour créer un repo par microservice
  for_each             = toset(var.service_names)
  name                 = "${var.project_name}-${each.value}"
  image_tag_mutability = "MUTABLE"

  image_scanning_configuration {
    scan_on_push = true
  }

  encryption_configuration {
    encryption_type = "AES256"
  }

  tags = {
    Name        = "${var.project_name}-${each.value}-ecr"
    Environment = var.env
    Service     = each.value
  }
}

# --- LIFECYCLE POLICY ---
# Applied to every repository. Tag-aware, matching what the fleet CI pushes
# (.github/workflows/fleet-images-deploy.yml): per-architecture images
# `:<git-sha>-amd64` / `:<git-sha>-arm64`, the multi-arch manifest `:<git-sha>`
# (the immutable deployment handle the staging overlay is pinned to) and a
# floating env tag on the same manifest (`:staging`; `:prod` for the promote
# flow; `:pr-validate` for manual validation runs).
#
# Rules are evaluated by priority and an image is only ever matched by the
# first rule that selects it, so the order below is the contract:
#   1. an image carrying an environment tag is NEVER expired — it is what a
#      cluster is pinned/promoted to, and a count-only rule would otherwise
#      expire it once enough newer images accumulate (the previous
#      "keep last 30 images, any tag" rule had exactly that failure mode);
#   2. per-arch images: keep the newest `retained_generations` per architecture;
#   3. the `:<git-sha>` manifests (and one-off tags): keep the newest
#      `retained_generations` generations. ECR counts a rule's `imageCountMoreThan`
#      over EVERY image its selection matches — including the per-arch images
#      already claimed by rules 2 — so the count is sized to a whole generation:
#      `retained_generations × (architectures + 1)` tagged images;
#   4. untagged images (a superseded floating tag, orphaned attestation /
#      child manifests) expire after `untagged_expiry_days`. Children still
#      referenced by a retained manifest are protected by ECR itself.
#
# One rule PER prefix / pattern: a `tagPrefixList` or `tagPatternList` with
# several entries selects only images carrying ALL of them (AND), not any of
# them — verified with `start-lifecycle-policy-preview` on 2026-09-16, where
# ["*-amd64", "*-arm64"] selected nothing. Rules 1–2 are therefore generated
# from the variables, one entry each, with computed priorities. No
# `tagStatus = any` rule: it would count fresh untagged children as the
# "newest" images and expire every older manifest (also seen in preview).
#
# Retention is thus expressed in fleet *generations* (one merge to develop =
# one generation), not in an opaque image count that also counted attestations
# and per-arch children. Preview on 2026-09-16 (post-server, 10 generations):
# 5 per-arch images per architecture + 5 manifests kept, the `staging` image
# among them, 7 untagged leftovers expired.
locals {
  ecr_protect_rules = [
    for i, prefix in var.protected_tag_prefixes : {
      rulePriority = i + 1
      description  = "Never expire an image an environment is pinned to (${prefix}*)"
      selection = {
        tagStatus     = "tagged"
        tagPrefixList = [prefix]
        countType     = "imageCountMoreThan"
        countNumber   = 10000
      }
      action = { type = "expire" }
    }
  ]

  ecr_arch_rules = [
    for i, arch in var.image_architectures : {
      rulePriority = length(var.protected_tag_prefixes) + i + 1
      description  = "Keep the newest ${var.retained_generations} ${arch} images (*-${arch})"
      selection = {
        tagStatus      = "tagged"
        tagPatternList = ["*-${arch}"]
        countType      = "imageCountMoreThan"
        countNumber    = var.retained_generations
      }
      action = { type = "expire" }
    }
  ]

  ecr_tail_rules = [
    {
      rulePriority = length(var.protected_tag_prefixes) + length(var.image_architectures) + 1
      description  = "Keep the newest ${var.retained_generations} generations of multi-arch manifests / other tags"
      selection = {
        tagStatus      = "tagged"
        tagPatternList = ["*"]
        countType      = "imageCountMoreThan"
        countNumber    = var.retained_generations * (length(var.image_architectures) + 1)
      }
      action = { type = "expire" }
    },
    {
      rulePriority = length(var.protected_tag_prefixes) + length(var.image_architectures) + 2
      description  = "Expire untagged images (superseded floating tags, orphaned children) after ${var.untagged_expiry_days} day(s)"
      selection = {
        tagStatus   = "untagged"
        countType   = "sinceImagePushed"
        countUnit   = "days"
        countNumber = var.untagged_expiry_days
      }
      action = { type = "expire" }
    },
  ]
}

resource "aws_ecr_lifecycle_policy" "cleanup" {
  for_each   = aws_ecr_repository.services
  repository = each.value.name

  policy = jsonencode({
    rules = concat(local.ecr_protect_rules, local.ecr_arch_rules, local.ecr_tail_rules)
  })
}
