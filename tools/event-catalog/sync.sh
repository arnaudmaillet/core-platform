#!/usr/bin/env bash
#
# sync.sh — keep the generated topic-wiring block in docs/domain/EVENT_CATALOG.md (and its
# translations) in lock-step with the event-topology registry.
#
# The block is rendered by the `gen-event-catalog` binary from the const tables in
# crates/contracts/event-topology; this script splices it between the BEGIN/END markers in each
# catalog doc. The wiring half of the catalog is therefore generated; the semantic half is authored.
#
# Usage:
#   tools/event-catalog/sync.sh --write   # regenerate the block in every catalog doc (in place)
#   tools/event-catalog/sync.sh --check   # fail if any doc's block is stale (CI gate)
#
# The same check is enforced by `cargo test -p event-topology` (generated_block_matches_registry_*).

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
BEGIN="<!-- BEGIN GENERATED: topic-wiring · source crates/contracts/event-topology · do not edit by hand -->"
END="<!-- END GENERATED: topic-wiring -->"
DOCS=(
  "$ROOT/docs/domain/EVENT_CATALOG.md"
  "$ROOT/docs/domain/EVENT_CATALOG.fr.md"
)

block_file="$(mktemp)"
trap 'rm -f "$block_file"' EXIT
cargo run -q -p event-topology --bin gen-event-catalog --manifest-path "$ROOT/Cargo.toml" > "$block_file"

# splice <doc> : replace the BEGIN..END region (inclusive) of <doc> with the generated block.
splice() {
  awk -v bf="$block_file" -v B="$BEGIN" -v E="$END" '
    BEGIN { while ((getline line < bf) > 0) blk = blk line "\n"; sub(/\n$/, "", blk) }
    index($0, B) { print blk; insec = 1; next }
    insec && index($0, E) { insec = 0; next }
    insec { next }
    { print }
  ' "$1"
}

mode="${1:---check}"
fail=0
for doc in "${DOCS[@]}"; do
  [ -f "$doc" ] || { echo "MISSING $doc" >&2; fail=1; continue; }
  if ! grep -qF "$BEGIN" "$doc"; then
    echo "NO MARKERS in $doc — add the $BEGIN / $END block once, then re-run." >&2; fail=1; continue
  fi
  case "$mode" in
    --write)
      tmp="$(mktemp)"; splice "$doc" > "$tmp" && mv "$tmp" "$doc"
      echo "wrote $doc"
      ;;
    --check)
      if ! diff -q <(splice "$doc") "$doc" >/dev/null; then
        echo "STALE  $doc — run tools/event-catalog/sync.sh --write" >&2; fail=1
      else
        echo "OK     $doc"
      fi
      ;;
    *) echo "usage: $0 {--write|--check}" >&2; exit 2 ;;
  esac
done

if [ "$mode" = "--write" ]; then
  echo "note: re-stamp the FR mirror after writing — tools/i18n/i18n-drift.sh stamp docs/domain/EVENT_CATALOG.fr.md"
fi
exit "$fail"
