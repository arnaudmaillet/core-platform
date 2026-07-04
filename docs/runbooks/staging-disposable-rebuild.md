# Runbook: Disposable staging — destroy → rebuild

Staging is a **disposable** environment: it is stood up from scratch, validated,
and torn down repeatedly. Most of that cycle is clean, but AWS **Secrets Manager**
and **KMS** carry *deletion state* that outlives a `terragrunt destroy` and can
block the next `apply`. This runbook captures the gotchas and the exact recovery.

## The core gotcha: name reservation after deletion

- A Secrets Manager secret in **`PendingDeletion`** reserves its name for the whole
  recovery window. `CreateSecret` with that name fails:
  `InvalidRequestException: You can't create this secret because a secret with this
  name is already scheduled for deletion.`
- **`describe-secret` returns `NotFound` for a `PendingDeletion` secret** — so a
  naive pre-flight ("does it exist?") reports the name as free when it is not. Use
  `aws secretsmanager list-secrets --include-planned-deletion`.
- Modules now set **`recovery_window_in_days = 0`** (PR #538), so teardowns delete
  secrets *immediately* rather than scheduling a 30-day window. But AWS still
  **reserves a force-deleted name for a few minutes**, so a rebuild started seconds
  after a destroy can still collide.
- **KMS:** MSK's SCRAM secret requires a customer-managed KMS key. Keys have a
  minimum **7-day** deletion window (no force-immediate). A teardown schedules the
  key for deletion; a rebuild creates a *fresh* key, so this is orphan cost, not a
  blocker — **unless** you `terraform import` a stale secret whose key is pending
  deletion, which then fails `UpdateSecret` with `KMSInvalidStateException`.
  **Prefer clearing (below) over importing.**

## Standard cycle

```bash
BASE=infrastructure/live/staging/us-east-1
# 1. Teardown
( cd $BASE && AWS_REGION=us-east-1 terragrunt run --all destroy --non-interactive -- -auto-approve )

# 2. Pre-flight the next apply: detect (and optionally clear) leftover name reservations.
bash infrastructure/assets/teardown/preflight-clean-env.sh staging            # report only
bash infrastructure/assets/teardown/preflight-clean-env.sh staging --fix      # clear leftovers

# 3. COOLDOWN: if --fix cleared anything, wait ~15 min so AWS releases the names.

# 4. Apply
( cd $BASE && AWS_REGION=us-east-1 GITHUB_TOKEN=$(gh auth token) \
    terragrunt run --all apply --non-interactive --backend-bootstrap -- -auto-approve )
```

> Trust the **Run Summary** (`Succeeded / Failed`), NOT the exit code — `terragrunt
> run --all` can exit 0 even with failed units.

## Recovery: a data-store unit fails with "already scheduled for deletion"

This means a secret name is still reserved. Two paths:

- **Clear (preferred):** `preflight-clean-env.sh staging --fix`, wait ~15 min, re-apply.
- **Adopt (no wait, but MSK-risky):** restore the secret and import it so Terraform
  manages it instead of re-creating:
  ```bash
  aws secretsmanager restore-secret --secret-id <name>
  ( cd $BASE/data/<unit> && terragrunt import aws_secretsmanager_secret.<res> <arn> )
  ```
  For **MSK** this hits `KMSInvalidStateException` if the imported secret's KMS key
  is pending deletion — re-enable it first:
  `aws kms cancel-key-deletion --key-id <id> && aws kms enable-key --key-id <id>`
  (then re-schedule its deletion after the apply). Because of this, **prefer Clear
  for MSK.**

## Teardown stale state (the ACM race)

If `eks` fails deleting its ACM certificate (`ResourceInUseException`, cert still
held by a not-yet-deprovisioned NLB) and `vpc` early-exits, the AWS resources are
usually gone but the unit *state* is stale. Reconcile per-unit:

```bash
( cd $BASE/eks && terragrunt state rm aws_acm_certificate.cert )
( cd $BASE/networking/vpc && terragrunt state rm $(cd $BASE/networking/vpc && terragrunt state list) )
```

The graceful-cleanup hook now waits for LBs to deprovision before Terraform reaches
the cert, so this should be rare. Always confirm zero leaks afterwards:
`aws ec2 describe-vpcs --filters Name=isDefault,Values=false` (ignore transient
list-flicker — verify by `--vpc-ids`).
