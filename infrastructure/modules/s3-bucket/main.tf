# infrastructure/modules/s3-bucket/main.tf
#
# Generic private S3 bucket, parameterized for the two object-store roles the
# fleet needs:
#
#   * media    — versioned, SSE-S3, CORS for browser presigned PUT/GET (bytes
#                never traverse gRPC/Kafka; clients upload/download direct).
#   * audit    — versioned + Object-Lock in COMPLIANCE mode (WORM: not even the
#                root account can delete/overwrite before retention), SSE-KMS
#                under the audit KEK. This is the external-witness anchor sink.
#
# Public access is always fully blocked; access is granted only via the
# consumer's IRSA policy (modules/security/irsa-roles), scoped to this ARN.

resource "aws_s3_bucket" "this" {
  bucket = var.name

  # Object Lock can ONLY be enabled at creation time and forces versioning on.
  object_lock_enabled = var.object_lock_mode != ""

  tags = var.tags
}

resource "aws_s3_bucket_versioning" "this" {
  bucket = aws_s3_bucket.this.id
  versioning_configuration {
    # Object Lock requires versioning; otherwise honor the flag.
    status = (var.object_lock_mode != "" || var.versioning_enabled) ? "Enabled" : "Suspended"
  }
}

resource "aws_s3_bucket_public_access_block" "this" {
  bucket                  = aws_s3_bucket.this.id
  block_public_acls       = true
  block_public_policy     = true
  ignore_public_acls      = true
  restrict_public_buckets = true
}

resource "aws_s3_bucket_server_side_encryption_configuration" "this" {
  bucket = aws_s3_bucket.this.id
  rule {
    apply_server_side_encryption_by_default {
      sse_algorithm     = var.kms_key_arn == "" ? "AES256" : "aws:kms"
      kms_master_key_id = var.kms_key_arn == "" ? null : var.kms_key_arn
    }
    bucket_key_enabled = var.kms_key_arn != ""
  }
}

# WORM retention — only when an Object-Lock mode is requested.
resource "aws_s3_bucket_object_lock_configuration" "this" {
  count  = var.object_lock_mode != "" ? 1 : 0
  bucket = aws_s3_bucket.this.id
  rule {
    default_retention {
      mode = var.object_lock_mode
      days = var.object_lock_retention_days
    }
  }
  depends_on = [aws_s3_bucket_versioning.this]
}

# CORS — only for browser-facing presigned flows (media).
resource "aws_s3_bucket_cors_configuration" "this" {
  count  = var.cors_enabled ? 1 : 0
  bucket = aws_s3_bucket.this.id
  cors_rule {
    allowed_methods = ["GET", "PUT", "HEAD"]
    allowed_origins = var.cors_allowed_origins
    allowed_headers = ["*"]
    expose_headers  = ["ETag"]
    max_age_seconds = 3000
  }
}
