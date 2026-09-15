#!/usr/bin/env bash
# infrastructure/assets/teardown/k8s-graceful-cleanup.sh
#
# `before_hook` (destroy) for the kubernetes/argocd Terragrunt unit. Drains the
# AWS resources that in-cluster controllers provision — and that Terraform does
# NOT own — BEFORE Terraform deletes the cluster and VPC. Without this:
#   * ALBs (Ingress) / NLBs (Service type=LoadBalancer) leak, and their leftover
#     ENIs / security groups block `aws_vpc` destroy with DependencyViolation;
#   * EBS volumes behind CNPG/Scylla PVCs leak (reclaimPolicy=Delete only fires
#     on orderly PVC deletion, which a blind destroy skips);
#   * Karpenter EC2 nodes leak (not in the managed node group Terraform deletes).
#
# ── Ordering is the whole point (2026-09-15 teardown post-mortem) ──────────────
# The previous version deleted LBs, CNPG/Scylla CRs and PVCs FIRST and the Argo
# CD Applications LAST. In the 3-minute window in between, the surviving apps'
# selfHeal recreated the ScyllaCluster, 12 CNPG PVCs and the public NLB, and
# Karpenter — still running, its Application deleted with
# preserveResourcesOnDeletion — kept re-provisioning nodes while this script
# `aws ec2 wait`ed on the OLD instances. Result: a new NLB (+ its two LBC
# security groups → VPC DependencyViolation), 14 orphan EBS volumes and 4 EC2
# instances, all cleaned by hand. Hence, now:
#   0. stop every reconciler first (Argo CD controllers, then Karpenter);
#   1. only then delete the cloud-backed objects, and WAIT for AWS to catch up
#      (LBs + their security groups, EBS volumes) before handing over to Terraform;
#   2. terminate Karpenter instances by tag (Karpenter is down, it cannot).
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

vpc_id="$(aws eks describe-cluster --name "${CLUSTER_NAME}" --region "${AWS_REGION}" \
  --query 'cluster.resourcesVpcConfig.vpcId' --output text 2>/dev/null || true)"
[ "${vpc_id:-None}" = "None" ] && vpc_id=""

# 0. STOP RECONCILIATION before deleting anything. Scaling the Argo CD
#    application controller to 0 halts every sync/selfHeal at once — deleting
#    ApplicationSets is not enough (app-of-apps children such as the fleet and
#    the ScyllaCluster app are not appset-owned and kept self-healing).
echo "Stopping Argo CD reconciliation (controllers -> 0)..."
kubectl scale statefulset argocd-application-controller -n argocd --replicas=0 --timeout=60s || true
kubectl scale deploy argocd-applicationset-controller -n argocd --replicas=0 --timeout=60s || true
kubectl patch app root-bootstrap -n argocd --type merge -p '{"spec":{"syncPolicy":null}}' || true
kubectl delete appsets --all -A --timeout=60s || true
# Apps are deleted for tidiness only; with the controller down their finalizers
# never run, so do not wait on them (the cluster is going away anyway).
kubectl delete app --all -n argocd --wait=false --timeout=30s || true

# 0b. Stop Karpenter too: with the controller down nothing re-provisions nodes
#     for pods orphaned by the deletions below. Its instances are terminated by
#     tag in step 5 (Karpenter cannot do it once scaled down).
echo "Stopping Karpenter (deploy -> 0)..."
kubectl scale deploy karpenter -n karpenter --replicas=0 --timeout=60s || true

# 1. Load balancers — frees the ALB (Ingress) and NLB (Service) ENIs that
#    otherwise block VPC destroy. The AWS LB controller (still running) does the
#    AWS-side deprovisioning asynchronously; we wait for it below.
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

# 1b. WAIT for the LBs to actually deprovision in AWS. `kubectl delete svc` returns
#     once the k8s object is gone, but the real NLB/ALB (+ENIs) goes away later,
#     which can take minutes. Without this the `eks` unit fails to delete its ACM
#     cert (still referenced by the NLB listener → ResourceInUseException) and
#     `vpc` early-exits on leftover ENIs. Best-effort, ~5 min.
if [ -n "${vpc_id}" ]; then
  echo "Waiting for load balancers in ${vpc_id} to deprovision..."
  for _ in $(seq 1 30); do
    remaining="$(aws elbv2 describe-load-balancers --region "${AWS_REGION}" \
      --query "length(LoadBalancers[?VpcId=='${vpc_id}'])" --output text 2>/dev/null || echo 0)"
    [ "${remaining:-0}" = "0" ] && { echo "  all load balancers gone"; break; }
    echo "  ${remaining} still deprovisioning..."; sleep 10
  done
  # 1c. The LB controller tags the security groups it creates for LBs
  #     (frontend + shared backend) with elbv2.k8s.aws/cluster=<cluster>. It
  #     removes them with the LB in the normal path, but any it left behind —
  #     e.g. an LB deleted out-of-band — is a guaranteed VPC DependencyViolation.
  #     The backend SG's rules reference the frontend SG, so revoke first.
  LBC_SGS="$(aws ec2 describe-security-groups --region "${AWS_REGION}" \
    --filters "Name=vpc-id,Values=${vpc_id}" "Name=tag:elbv2.k8s.aws/cluster,Values=${CLUSTER_NAME}" \
    --query 'SecurityGroups[].GroupId' --output text 2>/dev/null || true)"
  if [ -n "${LBC_SGS}" ]; then
    echo "Deleting leftover LB-controller security groups: ${LBC_SGS}"
    for sg in ${LBC_SGS}; do
      perms="$(aws ec2 describe-security-groups --region "${AWS_REGION}" --group-ids "$sg" \
        --query 'SecurityGroups[0].IpPermissions' --output json 2>/dev/null || echo '[]')"
      [ "${perms}" != "[]" ] && aws ec2 revoke-security-group-ingress --region "${AWS_REGION}" \
        --group-id "$sg" --ip-permissions "${perms}" >/dev/null 2>&1 || true
    done
    for sg in ${LBC_SGS}; do
      aws ec2 delete-security-group --region "${AWS_REGION}" --group-id "$sg" >/dev/null 2>&1 \
        && echo "  deleted ${sg}" || echo "  ${sg} still in use (retrying after the next steps)"
    done
  fi
fi

# 2. Stateful workloads: delete the CRs (removes pods that hold the volumes), then
#    PVCs, so ebs-csi issues DeleteVolume for the backing EBS volumes.
echo "Deleting CNPG / ScyllaDB clusters..."
kubectl delete clusters.postgresql.cnpg.io --all -A --timeout=180s || true
kubectl delete scyllaclusters.scylla.scylladb.com --all -A --timeout=300s || true

echo "Deleting PVCs (EBS volumes)..."
kubectl delete pvc --all -A --timeout=180s \
  || echo "PVC deletion timed out; continuing..."

# 2b. WAIT for the PVs to be reclaimed: DeleteVolume is issued by the EBS CSI
#     controller, which Terraform removes with the cluster — anything not gone by
#     then is an orphan volume. Best-effort, ~3 min; then a tag-scoped fallback.
echo "Waiting for PersistentVolumes to be reclaimed..."
for _ in $(seq 1 18); do
  left="$(kubectl get pv --no-headers 2>/dev/null | wc -l | tr -d ' ')"
  [ "${left:-0}" = "0" ] && { echo "  all PVs reclaimed"; break; }
  echo "  ${left} PV(s) still present..."; sleep 10
done
ORPHAN_VOLS="$(aws ec2 describe-volumes --region "${AWS_REGION}" \
  --filters "Name=tag:kubernetes.io/cluster/${CLUSTER_NAME},Values=owned" "Name=status,Values=available" \
  --query 'Volumes[].VolumeId' --output text 2>/dev/null || true)"
for v in ${ORPHAN_VOLS}; do
  aws ec2 delete-volume --region "${AWS_REGION}" --volume-id "$v" >/dev/null 2>&1 && echo "  deleted orphan volume ${v}" || true
done

# 3. Karpenter nodes. Karpenter is down (step 0b) so nodeclaim finalizers would
#    hang: delete the Node objects (fast, no wait) and terminate the instances
#    by tag. Karpenter (v1) tags every instance `karpenter.sh/nodepool` plus
#    `kubernetes.io/cluster/<cluster>=owned` — NOT `karpenter.sh/managed-by`.
#    MNG nodes carry the cluster tag but never the nodepool tag, so this cannot
#    touch them (Terraform deletes those with the node groups).
echo "Terminating Karpenter instances (EC2, by tag)..."
kubectl delete nodes -l karpenter.sh/nodepool --wait=false --timeout=30s || true
KARPENTER_INSTANCES="$(aws ec2 describe-instances --region "${AWS_REGION}" \
  --filters "Name=tag-key,Values=karpenter.sh/nodepool" \
            "Name=tag:kubernetes.io/cluster/${CLUSTER_NAME},Values=owned" \
            "Name=instance-state-name,Values=pending,running,stopping,stopped" \
  --query 'Reservations[].Instances[].InstanceId' --output text 2>/dev/null || true)"
if [ -n "${KARPENTER_INSTANCES}" ]; then
  echo "  terminating: ${KARPENTER_INSTANCES}"
  # shellcheck disable=SC2086
  aws ec2 terminate-instances --region "${AWS_REGION}" --instance-ids ${KARPENTER_INSTANCES} >/dev/null 2>&1 || true
  # shellcheck disable=SC2086
  aws ec2 wait instance-terminated --region "${AWS_REGION}" --instance-ids ${KARPENTER_INSTANCES} 2>/dev/null || true
else
  echo "  none."
fi

# 3b. Karpenter >= 1.7 creates one IAM instance profile per EC2NodeClass under
#     /karpenter/<region>/<cluster>/<nodeclass-uid>/ and deletes it when the
#     nodeclass goes — which never happens on a teardown (Karpenter is down).
#     Cost-free but they accumulate one per cycle; path-scoped to this cluster.
for ip in $(aws iam list-instance-profiles --path-prefix "/karpenter/${AWS_REGION}/${CLUSTER_NAME}/" \
    --query 'InstanceProfiles[].InstanceProfileName' --output text 2>/dev/null || true); do
  for role in $(aws iam get-instance-profile --instance-profile-name "$ip" \
      --query 'InstanceProfile.Roles[].RoleName' --output text 2>/dev/null || true); do
    aws iam remove-role-from-instance-profile --instance-profile-name "$ip" --role-name "$role" >/dev/null 2>&1 || true
  done
  aws iam delete-instance-profile --instance-profile-name "$ip" >/dev/null 2>&1 && echo "  deleted instance profile ${ip}" || true
done

# 4. Second pass on the LB-controller security groups: the first pass can fail
#    while the LB's ENIs were still attached; by now they are gone.
if [ -n "${vpc_id}" ]; then
  for sg in $(aws ec2 describe-security-groups --region "${AWS_REGION}" \
      --filters "Name=vpc-id,Values=${vpc_id}" "Name=tag:elbv2.k8s.aws/cluster,Values=${CLUSTER_NAME}" \
      --query 'SecurityGroups[].GroupId' --output text 2>/dev/null || true); do
    aws ec2 delete-security-group --region "${AWS_REGION}" --group-id "$sg" >/dev/null 2>&1 \
      && echo "  deleted ${sg}" || echo "  WARNING: ${sg} could not be deleted — expect a VPC DependencyViolation"
  done
fi

echo "--- Cleanup finished, proceeding to Terraform Destroy ---"
