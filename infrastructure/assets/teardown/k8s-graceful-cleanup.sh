#!/usr/bin/env bash
# infrastructure/assets/teardown/k8s-graceful-cleanup.sh
#
# `before_hook` (destroy) for the kubernetes/argocd Terragrunt unit. Drains the
# AWS resources that in-cluster controllers provision — and that Terraform does
# NOT own — BEFORE Terraform deletes the cluster and VPC. Without this:
#   * ALBs (Ingress) / NLBs (Service type=LoadBalancer) leak, and their leftover
#     ENIs block `aws_vpc` destroy with DependencyViolation;
#   * EBS volumes behind CNPG/Scylla PVCs leak (reclaimPolicy=Delete only fires
#     on orderly PVC deletion, which a blind destroy skips);
#   * Karpenter EC2 nodes leak (not in the managed node group Terraform deletes).
#
# Usage: k8s-graceful-cleanup.sh <cluster_name> <aws_region>
# Every step is best-effort (|| true): a partially-broken cluster must never block
# the destroy. Idempotent — safe to re-run.

set -uo pipefail

CLUSTER_NAME="${1:?cluster name required}"
AWS_REGION="${2:?aws region required}"

echo "--- Graceful Cleanup Start (${CLUSTER_NAME}) ---"

# Point kubectl at the target cluster (idempotent). If the cluster/API is already
# gone, every kubectl below no-ops via || true and we fall through to Terraform.
aws eks update-kubeconfig --name "${CLUSTER_NAME}" --region "${AWS_REGION}" >/dev/null 2>&1 \
  || echo "update-kubeconfig failed (cluster may already be gone); continuing..."

# 1. Stop ArgoCD reconciliation so it cannot recreate what we delete below.
echo "Disabling ArgoCD self-heal..."
kubectl patch app root-bootstrap -n argocd --type merge \
  -p '{"spec":{"syncPolicy":null}}' || true
kubectl delete appsets --all -A --timeout=60s || true

# 2. Load balancers FIRST — frees the ALB (Ingress) and NLB (Service) ENIs that
#    otherwise block VPC destroy. Wait for the AWS LB controller to deprovision.
echo "Deleting Ingresses (ALBs)..."
kubectl delete ingress --all -A --timeout=180s \
  || echo "Ingress deletion timed out; continuing..."

echo "Deleting type=LoadBalancer Services (NLBs)..."
# `spec.type` is not a supported field selector for Services, so enumerate.
kubectl get svc -A \
  -o go-template='{{range .items}}{{if eq .spec.type "LoadBalancer"}}{{.metadata.namespace}} {{.metadata.name}}{{"\n"}}{{end}}{{end}}' \
  2>/dev/null | while read -r ns name; do
  [ -n "${name:-}" ] && kubectl delete svc -n "$ns" "$name" --timeout=180s || true
done

# 2b. WAIT for the LBs to actually deprovision in AWS. `kubectl delete svc` returns
#     once the k8s object is gone, but the AWS LB controller then deletes the real
#     NLB/ALB asynchronously (+ENIs), which can take minutes. If we don't wait, the
#     later `eks` unit fails to delete its ACM cert (still referenced by the NLB's
#     TLS listener → ResourceInUseException) and `vpc` early-exits on leftover ENIs.
#     Poll until no load balancers remain in the cluster VPC (best-effort, ~5 min).
vpc_id="$(aws eks describe-cluster --name "${CLUSTER_NAME}" --region "${AWS_REGION}" \
  --query 'cluster.resourcesVpcConfig.vpcId' --output text 2>/dev/null || true)"
if [ -n "${vpc_id:-}" ] && [ "${vpc_id}" != "None" ]; then
  echo "Waiting for load balancers in ${vpc_id} to deprovision..."
  for _ in $(seq 1 30); do
    remaining="$(aws elbv2 describe-load-balancers --region "${AWS_REGION}" \
      --query "length(LoadBalancers[?VpcId=='${vpc_id}'])" --output text 2>/dev/null || echo 0)"
    [ "${remaining:-0}" = "0" ] && { echo "  all load balancers gone"; break; }
    echo "  ${remaining} still deprovisioning..."; sleep 10
  done
fi

# 3. Stateful workloads: delete the CRs (removes pods that hold the volumes), then
#    PVCs, so ebs-csi issues DeleteVolume for the backing EBS volumes.
echo "Deleting CNPG / ScyllaDB clusters..."
kubectl delete clusters.postgresql.cnpg.io --all -A --timeout=180s || true
kubectl delete scyllaclusters.scylla.scylladb.com --all -A --timeout=300s || true

echo "Deleting PVCs (EBS volumes)..."
kubectl delete pvc --all -A --timeout=180s \
  || echo "PVC deletion timed out; continuing..."

# 4. Stop the remaining ArgoCD apps EXCEPT Karpenter (kept alive for step 5) so
#    nothing re-syncs. ArgoCD tracks by label (not ownerRefs), so this halts drift;
#    the real cloud cleanup is steps 2-3 and 5.
echo "Deleting ArgoCD apps (excluding bootstrap + karpenter)..."
kubectl delete app -n argocd \
  -l "argocd.argoproj.io/instance!=root-bootstrap,app.kubernetes.io/name!=karpenter" \
  --cascade=foreground --timeout=180s || true

# 5. Drain Karpenter nodes WHILE Karpenter still runs, so it terminates the EC2
#    instances (and their ENIs/EBS) via the EC2 API.
echo "Draining Karpenter nodes (EC2)..."
kubectl delete nodeclaims --all --timeout=300s || true
kubectl delete nodes -l karpenter.sh/nodepool --timeout=180s || true

# 5b. Fallback: force-terminate any Karpenter instance the graceful drain left
#     behind. The kubectl delete above times out when many nodes exist (e.g. a
#     load-test scale-up) or Karpenter is already partway gone; the orphaned
#     instances then hold ENIs in the EKS node security group and hang
#     `aws_security_group.node` destroy for 10min+ (seen live 2026-07-04, 18
#     nodes). Karpenter tags every instance `karpenter.sh/managed-by=<cluster>`
#     (cluster-scoped + Karpenter-specific), so terminating by that tag can't
#     touch another cluster's nodes and guarantees no leak regardless of the
#     drain outcome above.
echo "Force-terminating any leftover Karpenter instances..."
LEFTOVER="$(aws ec2 describe-instances --region "${AWS_REGION}" \
  --filters "Name=tag:karpenter.sh/managed-by,Values=${CLUSTER_NAME}" \
            "Name=instance-state-name,Values=pending,running,stopping,stopped" \
  --query 'Reservations[].Instances[].InstanceId' --output text 2>/dev/null || true)"
if [ -n "${LEFTOVER}" ]; then
  echo "  terminating: ${LEFTOVER}"
  # shellcheck disable=SC2086
  aws ec2 terminate-instances --region "${AWS_REGION}" --instance-ids ${LEFTOVER} >/dev/null 2>&1 || true
  aws ec2 wait instance-terminated --region "${AWS_REGION}" --instance-ids ${LEFTOVER} 2>/dev/null || true
else
  echo "  none left."
fi

echo "--- Cleanup finished, proceeding to Terraform Destroy ---"
