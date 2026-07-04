#!/usr/bin/env bash
# infrastructure/assets/teardown/preflight-clean-env.sh
#
# Pre-apply hygiene check for a DISPOSABLE environment (staging). Detects the
# AWS Secrets Manager / KMS deletion-state debt that repeatedly blocks a
# destroy -> rebuild cycle, and (with --fix) clears it so the next apply doesn't
# collide with "secret ... is already scheduled for deletion".
#
# WHY THIS EXISTS (learned the hard way):
#   * A secret in PendingDeletion RESERVES its name for the whole recovery window
#     (up to 30 days). `describe-secret` returns NotFound for it — so a naive
#     pre-flight misses it; you MUST use `list-secrets --include-planned-deletion`.
#   * Modules now set recovery_window_in_days = 0 (PR #538) so teardowns delete
#     immediately — but AWS still RESERVES a force-deleted name for a few minutes,
#     so a rebuild kicked off seconds after a destroy can still collide. Run this
#     with a short cooldown before re-applying.
#   * MSK's SCRAM secret is encrypted with a customer KMS key; if you ever import
#     a stale copy of it whose key is pending deletion, UpdateSecret fails with
#     KMSInvalidStateException. Prefer clearing (this script) over import.
#
# Usage:
#   preflight-clean-env.sh <env> [--region <r>] [--fix]
#     (default: report only, read-only. --fix restores+force-deletes the leftovers.)

set -uo pipefail

ENV="${1:?usage: preflight-clean-env.sh <env> [--region <r>] [--fix]}"; shift || true
REGION="us-east-1"; FIX=0
while [ $# -gt 0 ]; do
  case "$1" in
    --region) REGION="$2"; shift 2 ;;
    --fix) FIX=1; shift ;;
    *) echo "unknown arg: $1"; exit 2 ;;
  esac
done

PREFIX="core-platform-${ENV}"
echo "--- Disposable-env preflight: env=${ENV} region=${REGION} fix=${FIX} ---"

# 1. Secrets in PendingDeletion whose name would collide with a rebuild.
echo "== Secrets Manager: names reserved by PendingDeletion =="
pending="$(aws secretsmanager list-secrets --include-planned-deletion --region "${REGION}" \
  --query "SecretList[?DeletedDate!=null && (starts_with(Name,'${PREFIX}') || starts_with(Name,'AmazonMSK_${PREFIX}'))].Name" \
  --output text 2>/dev/null || true)"

if [ -z "${pending// }" ]; then
  echo "  none — names are free."
else
  for s in $pending; do echo "  RESERVED: $s"; done
  if [ "$FIX" -eq 1 ]; then
    echo "  --fix: restoring then force-deleting to free the names..."
    for s in $pending; do
      aws secretsmanager restore-secret --secret-id "$s" --region "${REGION}" >/dev/null 2>&1 || true
      aws secretsmanager delete-secret --secret-id "$s" --region "${REGION}" \
        --force-delete-without-recovery >/dev/null 2>&1 \
        && echo "    freed: $s" || echo "    WARN could not free: $s"
    done
    echo "  NOTE: AWS reserves a force-deleted name for ~minutes. Wait ~15 min"
    echo "        before re-applying, or the CreateSecret will still collide."
  else
    echo "  Re-run with --fix to clear (or 'aws secretsmanager restore-secret' +"
    echo "  'delete-secret --force-delete-without-recovery' per name)."
  fi
fi

# 2. KMS keys in PendingDeletion (informational — a rebuild creates fresh keys, so
#    these are orphan cost, not a blocker; re-scheduling is harmless).
echo "== KMS: customer keys currently PendingDeletion (orphan-cost check) =="
keys="$(aws kms list-keys --region "${REGION}" --query "Keys[].KeyId" --output text 2>/dev/null || true)"
found_kms=0
for k in $keys; do
  st="$(aws kms describe-key --key-id "$k" --region "${REGION}" \
    --query "KeyMetadata.KeyState" --output text 2>/dev/null || echo "")"
  if [ "$st" = "PendingDeletion" ]; then echo "  PendingDeletion: $k"; found_kms=1; fi
done
[ "$found_kms" -eq 0 ] && echo "  none."

echo "--- preflight done ---"
