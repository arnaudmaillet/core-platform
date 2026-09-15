# infrastructure/modules/kubernetes/argocd/server/main.tf

# ── envsubst Config Management Plugin ──────────────────────────────────────────
# The Kustomize workload overlay (k8s/overlays/staging) carries ${VAR} runtime
# endpoints (MSK brokers, ElastiCache/OpenSearch endpoints, the ACM cert ARN) that
# are only known after the Terraform data-store apply. This plugin runs
# `kustomize build | <substitute>` inside argocd-repo-server, sourcing a
# Terraform-owned values file (Secret `cmp-envsubst-values`, written by the
# bootstrap module). Git stays the source of truth for the TEMPLATE; the CMP
# renders concrete manifests, so ArgoCD selfHeal is stable (no in-cluster edits to
# revert). Substitution is restricted to an explicit allow-list of variable names,
# so no other `$` in the manifests is ever touched.
#
# Implemented with `sed` over the exact `${VAR}` literals (no envsubst/gettext
# dependency — the argocd-cmp-server image ships sh + sed + kustomize).
locals {
  cmp_allowlist = [
    "MSK_BOOTSTRAP_BROKERS_SASL_SCRAM",
    "ELASTICACHE_CONFIG_ENDPOINT",
    "OPENSEARCH_ENDPOINT",
    "ACM_CERTIFICATE_ARN",
    "AUTH_JWKS_URL",
    "KEYCLOAK_TOKEN_ENDPOINT",
  ]

  cmp_generate_script = <<-SH
    set -a
    [ -f /cmp-values/env ] && . /cmp-values/env
    set +a
    out="$(kustomize build .)"
    for v in ${join(" ", local.cmp_allowlist)}; do
      val="$(printenv "$v" || true)"
      # Escape sed replacement metachars (\ & |) so AWS endpoints/ARNs are inserted literally.
      esc="$(printf '%s' "$val" | sed -e 's/[\\&|]/\\&/g')"
      out="$(printf '%s' "$out" | sed "s|\$${$v}|$esc|g")"
    done
    printf '%s\n' "$out"
  SH
}

# The plugin ConfigMap is rendered as part of the Helm release (extraObjects), not
# a separate kubernetes_config_map — that keeps it in the chart's namespace and
# sync order (no Terraform dependency cycle with the release, and the repo-server
# pod finds it on first start).
locals {
  cmp_plugin_configmap = {
    apiVersion = "v1"
    kind       = "ConfigMap"
    metadata = {
      name      = "argocd-cmp-envsubst"
      namespace = "argocd"
    }
    data = {
      "plugin.yaml" = yamlencode({
        apiVersion = "argoproj.io/v1alpha1"
        kind       = "ConfigManagementPlugin"
        # No `discover` block: the plugin activates ONLY when an Application names
        # it (spec.source.plugin.name: envsubst), so it never hijacks the legacy
        # dev Helm apps.
        metadata = { name = "envsubst" }
        spec = {
          version = "v1.0"
          generate = {
            command = ["/bin/sh", "-c"]
            args    = [local.cmp_generate_script]
          }
        }
      })
    }
  }
}

# The argo-cd chart version -> Argo CD app version the sidecar below must match.
# Bumping `argocd_version` without adding its entry here fails the plan on the
# precondition instead of silently running a mismatched cmp-server.
locals {
  argocd_chart_app_version = {
    "7.7.0"  = "v2.13.0"
    "10.9.1" = "v3.5.3"
  }
}

resource "helm_release" "argocd" {
  lifecycle {
    precondition {
      condition     = contains(keys(local.argocd_chart_app_version), var.argocd_version)
      error_message = "argocd_version ${var.argocd_version} has no app-version entry in local.argocd_chart_app_version (modules/kubernetes/argocd/server/main.tf); add it (chart appVersion) so the cmp-envsubst sidecar image stays in lockstep."
    }
  }

  name             = "argocd"
  repository       = "https://argoproj.github.io/argo-helm"
  chart            = "argo-cd"
  namespace        = "argocd"
  create_namespace = true
  version          = var.argocd_version
  cleanup_on_fail  = true
  wait             = true
  timeout          = 300

  values = [
    yamlencode({
      commonLabels = {
        "argocd.argoproj.io/managed-by" = "helm"
      }
      server = {
        # TLS terminates at the ALB; the server speaks plain HTTP behind it.
        extraArgs = ["--insecure"]
        service   = { type = "ClusterIP" }
      }
      # Canonical home of the flag (argocd-cmd-params-cm). Chart >= 9 ships no
      # default params any more, so it is stated explicitly.
      configs = {
        params = {
          "server.insecure" = true
        }
      }
      global = {
        # Chart >= 10 creates NetworkPolicies in the argocd namespace by default.
        # The ALB reaches argocd-server pods from OUTSIDE the pod network (IP
        # targets) and the VPC CNI enforces NetworkPolicy, so that default would
        # black-hole the UI/API. Ingress isolation for the fleet lives in the
        # workload overlays, not here.
        networkPolicy = { create = false }
      }
      redis = { enabled = true }

      # The CMP plugin config, rendered with the release (see local above).
      extraObjects = [local.cmp_plugin_configmap]

      # ── envsubst CMP sidecar on the repo-server ──────────────────────────────
      repoServer = {
        # The sidecar runs argocd-cmp-server with our plugin config; it shares the
        # repo-server's `var-files`/`plugins` volumes (chart defaults) and mounts
        # the plugin config + the Terraform values Secret.
        extraContainers = [
          {
            name = "cmp-envsubst"
            # Tracks the argo-cd chart's appVersion via local.argocd_chart_app_version
            # (precondition-guarded). The sidecar ships argocd-cmp-server + kustomize.
            image   = "quay.io/argoproj/argocd:${local.argocd_chart_app_version[var.argocd_version]}"
            command = ["/var/run/argocd/argocd-cmp-server"]
            securityContext = {
              runAsNonRoot             = true
              runAsUser                = 999
              allowPrivilegeEscalation = false
              readOnlyRootFilesystem   = true
              capabilities             = { drop = ["ALL"] }
              seccompProfile           = { type = "RuntimeDefault" }
            }
            volumeMounts = [
              { mountPath = "/var/run/argocd", name = "var-files" },
              { mountPath = "/home/argocd/cmp-server/plugins", name = "plugins" },
              { mountPath = "/home/argocd/cmp-server/config/plugin.yaml", subPath = "plugin.yaml", name = "cmp-envsubst-plugin" },
              { mountPath = "/cmp-values", name = "cmp-envsubst-values", readOnly = true },
              { mountPath = "/tmp", name = "cmp-tmp" },
            ]
          }
        ]
        volumes = [
          { name = "cmp-envsubst-plugin", configMap = { name = "argocd-cmp-envsubst" } },
          # optional: render proceeds with empty vars until the Secret exists
          # (e.g. before the data-store apply, or on dev which omits the overlay).
          { name = "cmp-envsubst-values", secret = { secretName = "cmp-envsubst-values", optional = true } },
          { name = "cmp-tmp", emptyDir = {} },
        ]
      }
    })
  ]
}
