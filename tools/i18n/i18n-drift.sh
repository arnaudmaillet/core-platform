#!/usr/bin/env bash
#
# i18n-drift.sh — detect drift between canonical docs and their translations.
#
# A translation (<name>.<lang>.md, e.g. README.fr.md, DOMAIN.fr.md, CONTEXT_MAP.fr.md) records,
# in its YAML frontmatter, the SHA-256 of the canonical <name>.md it was translated from. This
# script recomputes that hash and flags any translation whose recorded hash no longer matches —
# unless it is explicitly `status: stale`.
#
# Usage:
#   tools/i18n/i18n-drift.sh check               # verify all translations (CI gate)
#   tools/i18n/i18n-drift.sh stamp <name>.fr.md  # record current source hash into a translation
#
# See docs/i18n/TRANSLATION.md for the full standard.

set -euo pipefail

# --- portable SHA-256 (Linux: sha256sum, macOS: shasum) -----------------------------------
sha() {
  if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1" | awk '{print $1}'
  else shasum -a 256 "$1" | awk '{print $1}'; fi
}

# --- extract a scalar from the leading YAML frontmatter: frontmatter_get <file> <key> ------
# Matches the key anywhere inside the first `---`…`---` block, at any indent (handles nesting).
frontmatter_get() {
  awk -v key="$2" '
    BEGIN { inblk = 0 }
    NR == 1 && $0 !~ /^---[[:space:]]*$/ { exit }     # no frontmatter
    /^---[[:space:]]*$/ { inblk++; if (inblk == 2) exit; next }
    inblk == 1 && $0 ~ "^[[:space:]]*" key ":" {
      sub("^[[:space:]]*" key ":[[:space:]]*", ""); gsub(/[[:space:]]+$/, ""); print; exit
    }
  ' "$1"
}

# --- discover all translation files (<name>.<lang>.md, two-letter lang code) ----------------
# Matches README.fr.md, DOMAIN.fr.md, CONTEXT_MAP.fr.md, … but never a canonical <name>.md
# (which has no .<lang>. segment). The canonical sibling is derived by stripping .<lang>.md.
find_translations() {
  find . -path ./target -prune -o -type f -name '*.[a-z][a-z].md' -print | sort
}

# --- canonical source for a translation: strip the .<lang>.md suffix, append .md -----------
canonical_for() { printf '%s.md\n' "${1%.[a-z][a-z].md}"; }

check() {
  local fail=0 found=0 fr src want status got
  while IFS= read -r fr; do
    [ -n "$fr" ] || continue
    found=1
    src="$(canonical_for "$fr")"
    if [ ! -f "$src" ]; then
      printf 'FAIL  %s\n        no canonical source %s\n' "$fr" "$src"; fail=1; continue
    fi
    want="$(frontmatter_get "$fr" source_sha256 || true)"
    status="$(frontmatter_get "$fr" status || true)"
    got="$(sha "$src")"
    if [ -z "$want" ]; then
      printf 'FAIL  %s\n        missing i18n.source_sha256 frontmatter\n' "$fr"; fail=1
    elif [ "$want" = "$got" ]; then
      printf 'OK    %s\n' "$fr"
    elif [ "$status" = "stale" ]; then
      printf 'WARN  %s\n        source changed; translation marked stale (acknowledged debt)\n' "$fr"
    else
      printf 'FAIL  %s\n        stale: recorded %s, source is now %s\n        retranslate and re-stamp, or set status: stale\n' \
        "$fr" "$want" "$got"; fail=1
    fi
  done < <(find_translations)

  [ "$found" = 1 ] || echo "no translation files found (nothing to check)"
  if [ "$fail" != 0 ]; then echo; echo "i18n drift detected — see FAIL lines above."; fi
  return "$fail"
}

stamp() {
  local fr="${1:-}"
  [ -n "$fr" ] && [ -f "$fr" ] || { echo "usage: $0 stamp <README.<lang>.md>" >&2; exit 2; }
  local src want today tmp
  src="$(canonical_for "$fr")"
  [ -f "$src" ] || { echo "no canonical source $src for $fr" >&2; exit 2; }
  want="$(sha "$src")"; today="$(date +%F)"; tmp="$(mktemp)"
  awk -v h="$want" -v d="$today" '
    /^---[[:space:]]*$/ { blk++ }
    blk == 1 && /^[[:space:]]*source_sha256:/ { sub(/source_sha256:.*/, "source_sha256: " h);     print; next }
    blk == 1 && /^[[:space:]]*translated_at:/ { sub(/translated_at:.*/,  "translated_at: " d);     print; next }
    blk == 1 && /^[[:space:]]*status:/        { sub(/status:.*/,         "status: complete");       print; next }
    { print }
  ' "$fr" > "$tmp" && mv "$tmp" "$fr"
  echo "stamped $fr -> source_sha256 $want ($today, status: complete)"
}

case "${1:-check}" in
  check) check ;;
  stamp) stamp "${2:-}" ;;
  *) echo "usage: $0 {check|stamp <file>}" >&2; exit 2 ;;
esac
