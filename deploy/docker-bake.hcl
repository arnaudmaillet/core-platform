# deploy/docker-bake.hcl
#
# Packages PREBUILT fleet binaries into runtime images — the CI path
# (.github/workflows/fleet-images-deploy.yml explains the why).
#
# For every binary in FLEET_BINS this builds ONLY the `runtime` stage of
# deploy/Dockerfile, overriding its `builder` stage with the local directory
# dist/<bin>/ — a BuildKit named context — where CI has installed the compiled
# binary at /usr/local/bin/app. `COPY --from=builder /usr/local/bin/app` in the
# Dockerfile then resolves against that directory, so cargo, cargo-chef and the
# chef/planner/builder stages never run here. The Dockerfile stays the single
# definition of what ships (base image, packages, the ffmpeg conditional for
# the media binaries, the non-root user): both routes produce the same image.
#
# One `bake` invocation = one BuildKit session for all binaries: the shared
# runtime layers (debian-slim + apt) are built once and reused; each image is
# then a COPY of its binary. Seconds, not minutes.
#
#   FLEET_BINS="chat-server migrator" REG=<registry> SHA=<git-sha> \
#   ARCH=amd64 PLATFORM=linux/amd64 \
#     docker buildx bake -f deploy/docker-bake.hcl --push
#
# Paths: every relative path here resolves against the INVOKING directory (the
# repo root, in CI and in the example above), not against this file. The main
# context is deploy/ (tiny — the runtime stage copies nothing from it); the
# named `builder` contexts are the dist/<bin>/ directories at the repo root.

variable "FLEET_BINS" { default = "" }      # space-separated binary names
variable "REG"        { default = "local" } # registry host (ECR in CI)
variable "SHA"        { default = "dev" }   # immutable tag = git sha
variable "ARCH"       { default = "amd64" }
variable "PLATFORM"   { default = "linux/amd64" }

group "default" {
  targets = ["fleet"]
}

target "fleet" {
  name = "img-${bin}"
  matrix = {
    bin = split(" ", trimspace(FLEET_BINS))
  }
  context    = "deploy"
  dockerfile = "Dockerfile"
  target     = "runtime"
  contexts = {
    builder = "dist/${bin}"
  }
  args = {
    BIN = bin
  }
  platforms = [PLATFORM]
  tags      = ["${REG}/core-platform-${bin}:${SHA}-${ARCH}"]
}
